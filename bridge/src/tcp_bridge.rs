use bytemuck::{Pod, Zeroable};
use std::{
    collections::HashMap,
    fmt::Debug,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use std::sync::atomic::AtomicUsize;

use bytes::Bytes;
use serialize::{Communicate, UseCast};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::{
        mpsc::{self, UnboundedReceiver},
        oneshot,
    },
};
use tracing::{debug, info, trace};

use crate::id_tracker::{ExchangeId, RecvId, REGISTER_MESSAGE_ID, SendId};

type Error = crate::BridgeError;
type Result<T> = std::result::Result<T, Error>;

const CLIENT_TCP_BUFFER_SIZE: usize = 1024 * 32;

/// Wrapper for TCP Connection that can be shared safely.
/// Each message will have a message ID, and user can subscribe the message ID
/// to get an message. For now, the message queue is unbounded.
#[derive(Debug, Clone)]
pub struct TcpConnection {
    /// User can send message to peer using this mpsc queue. This includes
    /// message id, message content, and a signal sender to indicate complete.
    write_channel: mpsc::UnboundedSender<(SendId, Bytes, oneshot::Sender<()>)>,
    /// User can subscribe a message using a message id, and the receiver
    /// channel will return bytes
    subscribe_channel: mpsc::UnboundedSender<(RecvId, oneshot::Sender<Bytes>)>,
    num_bytes_recv: Arc<AtomicUsize>,
    socket_addr: SocketAddr,
    uid: ClientID
}

struct PendingBuffer {
    pending_subscribe: HashMap<RecvId, oneshot::Sender<Bytes>>,
    pending_message: HashMap<RecvId, Bytes>,
}

impl PendingBuffer {
    fn new() -> Self {
        PendingBuffer {
            pending_subscribe: HashMap::new(),
            pending_message: HashMap::new(),
        }
    }
}

impl TcpConnection {
    fn new(socket: TcpStream, uid: ClientID) -> Self {
        let socket_addr = socket.peer_addr().unwrap();

        let (read_socket, write_socket) = socket.into_split();
        let (write_sender, write_receiver) = mpsc::unbounded_channel();
        let (subscribe_sender, subscribe_receiver) = mpsc::unbounded_channel();
        let pending_buffer = Arc::new(Mutex::new(PendingBuffer::new()));

        let num_recv_bytes = Arc::new(AtomicUsize::new(0));

        // read loop
        {
            let pending_buffer = pending_buffer.clone();
            let num_bytes_recv = num_recv_bytes.clone();
            tokio::spawn(async move {
                let mut read_socket = BufReader::with_capacity(CLIENT_TCP_BUFFER_SIZE, read_socket);
                loop {
                    let (message_id, read_buffer) = match read_one_message(&mut read_socket).await {
                        Ok(message) => message,
                        Err(e) => {
                            trace!("read_one_message error: {:?}", e);
                            break;
                        }
                    };
                    let read_buffer_len = read_buffer.len();
                    num_bytes_recv.fetch_add(read_buffer_len, std::sync::atomic::Ordering::Relaxed);
                    {
                        let mut pending = pending_buffer.lock().unwrap();
                        // if there is pending subscribe, send the message to pending subscribe
                        // channel
                        if let Some(v) = pending.pending_subscribe.remove(&message_id) {
                            if let Err(_) = v.send(read_buffer) {
                                debug!("subscribe reader is dead")
                            };
                            trace!(
                                "done read buffer of size: {}, id: {}, satisfy to pending subscribe",
                                read_buffer_len,
                                message_id
                            );
                            continue;
                        } else {
                            pending.pending_message.insert(message_id, read_buffer);
                            trace!(
                                "done read buffer of size: {}, id: {}, push to pending message",
                                read_buffer_len,
                                message_id
                            );
                        }
                    }
                }
            });
        }

        // subscribe loop
        tokio::spawn(async move {
            let mut subscribe: UnboundedReceiver<(RecvId, oneshot::Sender<Bytes>)> =
                subscribe_receiver;
            while let Some((message_id, callback)) = subscribe.recv().await {
                let mut pending = pending_buffer.lock().unwrap();

                if let Some(v) = pending.pending_message.remove(&message_id) {
                    // if there is message pending for this subscribe, get it
                    trace!("found subscribed data: id={}", message_id.0);
                    let callback: oneshot::Sender<Bytes> = callback;
                    if let Err(_) = callback.send(v) {
                        debug!("subscribe reader is dead");
                        return;
                    };
                    continue;
                } else {
                    // if there is not: add them to pending subscription
                    trace!(
                        "not found subscribed data: id={}, put to pending subscribe",
                        message_id.0
                    );
                    pending.pending_subscribe.insert(message_id, callback);
                }
            }
            trace!("all holders for the TCP connection is out of scope. subscribe loop quit");
        });

        // write loop
        {
            let mut write_receiver: UnboundedReceiver<(SendId, Bytes, oneshot::Sender<()>)> =
                write_receiver;
            // TODO: we need to return a handle to this to make sure the write loop is
            // killed when we quit
            // TODO: we can remove mpsc completely. See MpcConnection.
            tokio::spawn(async move {
                let mut write_socket = BufWriter::with_capacity(CLIENT_TCP_BUFFER_SIZE, write_socket);
                while let Some((message_id, data, complete)) = write_receiver.recv().await {
                    write_one_message_without_flush(&mut write_socket, message_id, data)
                        .await
                        .unwrap();
                    write_socket.flush().await.unwrap();
                    complete.send(()).map_or((), |_| {});
                }
                debug!("all holders for the TCP connection is out of scope, and there is not remaining data to send, so write loop quit");
            });
        }

        Self {
            write_channel: write_sender,
            subscribe_channel: subscribe_sender,
            num_bytes_recv: num_recv_bytes,
            socket_addr,
            uid
        }
    }

    /// Initialize a new connection with the given socket and uid. Return a connection and a channel indicating if registration message is successfully sent.
    pub fn new_client_side(socket: TcpStream, uid: ClientID) -> (Self, oneshot::Receiver<()>) {
        let conn = Self::new(socket, uid);
        let chan = register_to_server(&conn, uid).unwrap();
        (conn, chan)
    }

    /// Initialize a new connection with the given socket, receive the registration message, and return a connection asynchronously.
    pub async fn new_server_side(socket: TcpStream) -> Self {
        let mut conn = Self::new(socket, ClientID::default());
        let client_id = conn
            .subscribe_and_get::<UseCast<ClientID>>(RecvId(REGISTER_MESSAGE_ID))
            .await
            .unwrap();
        conn.uid = client_id;
        conn
    }

    /// Get statistics of how many bytes received from the peer,
    pub fn num_bytes_received(&self) -> usize {
        self.num_bytes_recv.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn socket_addr(&self) -> SocketAddr {
        self.socket_addr
    }

    pub fn uid(&self) -> ClientID {
        self.uid
    }

    /// Send message to peer. Return a receiver to get complete state.
    pub fn send_message_bytes(&self, id: SendId, message: Bytes) -> oneshot::Receiver<()> {
        let (sig_sender, sig_receiver) = oneshot::channel::<()>();
        self.write_channel
            .send((id, message, sig_sender))
            .unwrap_or_else(|_| { /*no-op*/ });
        sig_receiver
    }

    pub async fn subscribe_and_get_bytes(&self, id: RecvId) -> Bytes {
        // create a one-shot channel
        let (sender, receiver) = oneshot::channel();
        self.subscribe_channel.send((id, sender)).unwrap();
        receiver.await.unwrap()
    }

    pub fn send_message<M: Communicate>(
        &self,
        id: SendId,
        msg: M,
    ) -> Result<oneshot::Receiver<()>> {
        Ok(self.send_message_bytes(id, msg.into_bytes_owned()))
    }

    pub async fn subscribe_and_get<M: Communicate>(&self, id: RecvId) -> Result<M::Deserialized> {
        let data = self.subscribe_and_get_bytes(id).await;
        let msg = M::from_bytes_owned(data)?;
        Ok(msg)
    }

    pub async fn exchange_message<M: Communicate>(
        &self,
        id: ExchangeId,
        msg: M,
    ) -> Result<M::Deserialized> {
        self.send_message(id.send_id, msg)?;
        self.subscribe_and_get::<M>(id.recv_id).await
    }
}

fn register_to_server(conn: &TcpConnection, id: ClientID) -> Result<oneshot::Receiver<()>> {
    conn.send_message(SendId(REGISTER_MESSAGE_ID), &UseCast(id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Pod, Zeroable)]
#[repr(transparent)]
pub struct ClientID {
    pub id: u64,
}

impl ClientID {
    pub fn is_odd(&self) -> bool {
        self.id & 1 == 1
    }

    pub fn is_even(&self) -> bool {
        self.id & 1 == 0
    }
}

impl From<u64> for ClientID {
    fn from(id: u64) -> Self {
        Self { id }
    }
}

impl ClientID {
    pub fn new(id: u64) -> Self {
        Self { id }
    }
}

/// Make two tcp connection on localhost
pub async fn localhost_pair(port: u16) -> (TcpConnection, TcpConnection) {
    let server_handle = tokio::spawn(async move {
        let listener = TcpListener::bind(("localhost", port)).await.unwrap();

        info!("Listening to {}", port);
        let (socket, _) = listener.accept().await.unwrap();
        info!(
            "connection established: {}<->{}",
            port,
            socket.peer_addr().unwrap()
        );
        let conn = TcpConnection::new_server_side(socket).await;
        conn
    });

    let client_handle = tokio::spawn(async move {
        let socket;
        loop {
            match TcpStream::connect(("localhost", port)).await {
                Ok(s) => {
                    socket = s;
                    break;
                }
                Err(_) => {
                    debug!("waiting to connecting in 10ms");
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }
        }
        let (conn, handle) = TcpConnection::new_client_side(socket, ClientID::default());
        handle.await.unwrap();
        conn
    });

    let (server_handle, client_handle) = tokio::join!(server_handle, client_handle);
    (server_handle.expect("server panics"),
     client_handle.expect("client panics"))
}

pub(crate) async fn read_one_message(
    read_socket: &mut BufReader<OwnedReadHalf>,
) -> Result<(RecvId, Bytes)> {
    trace!("try read header");
    // receive header
    let message_id = read_socket.read_u64_le().await?;
    let message_size = read_socket.read_u64_le().await?;

    trace!("done read header, id: {}", message_id);
    trace!(
        "try read buffer: message_size: {}, id: {}",
        message_size,
        message_id
    );
    let mut read_buffer = bytes::BytesMut::with_capacity(message_size as usize);
    while read_buffer.len() < read_buffer.capacity() {
        read_socket.read_buf(&mut read_buffer).await?;
    }

    Ok((message_id.into(), read_buffer.freeze()))
}

pub(crate) async fn write_one_message_without_flush(
    write_socket: &mut BufWriter<OwnedWriteHalf>,
    message_id: SendId,
    mut data: Bytes,
) -> Result<()> {
    // write header
    trace!("try write header, id: {}", message_id.0);
    write_socket.write_u64_le(message_id.0).await?;
    write_socket.write_u64_le(data.len() as u64).await?;

    trace!("done write header, id: {}", message_id.0);
    trace!(
        "try write buffer with size: {:?}, id: {}",
        data.len(),
        message_id.0
    );
    // write message
    write_socket.write_all_buf(&mut data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_derive::{Deserialize, Serialize};
    use serialize::UseSerde;
    use tracing::info;

    use crate::id_tracker::IdGen;

    use super::localhost_pair;

    #[derive(Serialize, Deserialize, Eq, PartialEq, Clone, Debug)]
    struct HelloWorldMessage {
        msg: String,
        num: u128,
    }

    fn make_test_message() -> HelloWorldMessage {
        HelloWorldMessage {
            msg: "Hello World!!".into(),
            num: 0xdeadbeefabcdabcdaabbccddeeff1234,
        }
    }

    const TEST_PORT: u16 = 6665;

    #[tokio::test]
    #[ignore]
    async fn test_bridge() {
        // tracing_subscriber::fmt()
        //     .pretty()
        //     .with_max_level(Level::INFO)
        //     .init();

        let (server, client) = localhost_pair(TEST_PORT).await;
        let server_handle = tokio::spawn(async move {
            let data = make_test_message();
            server.send_message(12.into(), &UseSerde(data)).unwrap();
            info!("Message sent");
            server
        });

        let client_handle = tokio::spawn(async move {
            let data = client
                .subscribe_and_get::<UseSerde<HelloWorldMessage>>(12.into())
                .await
                .unwrap();
            assert_eq!(data, make_test_message());
            info!("got message ok!");
            client
        });

        let conn1 = server_handle.await.unwrap();
        let conn2 = client_handle.await.unwrap();
        drop(conn1);
        drop(conn2);
    }

    #[tokio::test]
    #[ignore]
    async fn test_exchange() {
        let msg1 = vec![11u32, 22, 33, 44];
        let msg2 = vec![55u32, 66, 77, 88];

        let expected1 = msg1.clone();
        let expected2 = msg2.clone();

        let (server1, server2) = localhost_pair(TEST_PORT).await;
        let server1_handle = tokio::spawn(async move {
            let received1 = server1.exchange_message(12.into(), &msg1).await.unwrap();
            (received1, server1)
        });

        let server2_handle = tokio::spawn(async move {
            let received2 = server2.exchange_message(12.into(), &msg2).await.unwrap();
            (received2, server2)
        });

        let (actual2, _) = server1_handle.await.unwrap();
        let (actual1, _) = server2_handle.await.unwrap();

        assert_eq!(expected1, actual1);
        assert_eq!(expected2, actual2);
    }

    #[tokio::test]
    #[ignore]
    /// Make sure message sending order does not matter.
    async fn test_exchange_using_reserve() {
        let msg1 = vec![11u32, 22, 33, 44];
        let msg2 = vec![55u32, 66, 77, 88];
        let msg3 = vec![99u32, 10, 11, 12];
        let msg4 = vec![13u32, 14, 15, 16];

        let expected1 = msg1.clone();
        let expected2 = msg2.clone();
        let expected3 = msg3.clone();
        let expected4 = msg4.clone();

        let (server1, server2) = localhost_pair(TEST_PORT).await;
        let server1_handle = tokio::spawn(async move {
            let mut message_id = IdGen::new();
            let mut mid_for_first = message_id.reserve_rounds(10);
            let server1_cloned = server1.clone();
            let received4 = tokio::task::spawn(async move {
                server1_cloned
                    .exchange_message(message_id.next_exchange_id(), &msg2)
                    .await
                    .unwrap()
            });
            let received3 = server1
                .exchange_message(mid_for_first.next_exchange_id(), &msg1)
                .await
                .unwrap();
            (received3, received4.await.unwrap(), server1)
        });

        let server2_handle = tokio::spawn(async move {
            let mut message_id = IdGen::new();
            let mut mid_for_first = message_id.reserve_rounds(10);
            let received1 = server2
                .exchange_message(mid_for_first.next_exchange_id(), &msg3)
                .await
                .unwrap();
            let received2 = server2
                .exchange_message(message_id.next_exchange_id(), &msg4)
                .await
                .unwrap();
            (received1, received2, server2)
        });

        let (actual3, actual4, _) = server1_handle.await.unwrap();
        let (actual1, actual2, _) = server2_handle.await.unwrap();

        assert_eq!(expected1, actual1);
        assert_eq!(expected2, actual2);
        assert_eq!(expected3, actual3);
        assert_eq!(expected4, actual4);
    }

    #[cfg(feature = "optional_tests")]
    #[tokio::test]
    #[ignore]
    async fn exchange_benchmark() {
        use std::time;
        const NUM_BYTES: usize = 500000000;
        let msg1 = vec![1u8; NUM_BYTES];
        let msg2 = vec![2u8; NUM_BYTES];
        let msg3 = vec![3u8; NUM_BYTES];
        let msg4 = vec![4u8; NUM_BYTES];

        let (server1, server2) = localhost_pair(TEST_PORT).await;
        let server1_handle = tokio::spawn(async move {
            let received1 = server1.exchange_message(12.into(), &msg1).await.unwrap();
            (received1, server1)
        });

        let server2_handle = tokio::spawn(async move {
            let received2 = server2.exchange_message(12.into(), &msg2).await.unwrap();
            (received2, server2)
        });

        let (server3, server4) = localhost_pair(TEST_PORT + 1).await;
        let server3_handle = tokio::spawn(async move {
            let received3 = server3.exchange_message(12.into(), &msg3).await.unwrap();
            (received3, server3)
        });

        let server4_handle = tokio::spawn(async move {
            let received4 = server4.exchange_message(12.into(), &msg4).await.unwrap();
            (received4, server4)
        });

        let t0 = time::Instant::now();
        let (..) = server1_handle.await.unwrap();
        let (..) = server2_handle.await.unwrap();
        let (..) = server3_handle.await.unwrap();
        let (..) = server4_handle.await.unwrap();
        let t1 = time::Instant::now();

        let speed = (NUM_BYTES * 2) as f64 / (t1 - t0).as_secs_f64() / 1000000.;

        println!("Exchange speed: {} MB/s", speed * 2.);
    }
}
