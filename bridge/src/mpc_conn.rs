use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Debug,
    net::IpAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use std::str::FromStr;

use bytes::Bytes;
use serialize::Communicate;
use tokio::{
    io::{AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::oneshot,
};
use tracing::{debug, info, trace};

use crate::{BlackBox, id_tracker::{ExchangeId, RecvId, SendId}, tcp_bridge::{read_one_message, write_one_message_without_flush}, tcp_connect_or_retry};

type Error = crate::BridgeError;
type Result<T> = std::result::Result<T, Error>;

const MPC_TCP_BUFFER_SIZE: usize = 1024 * 1024;

/// `Upcoming` contains either the data, or a channel to receive the upcoming
/// data.
pub enum Upcoming<T> {
    Ready(T),
    Wait(oneshot::Receiver<T>),
}

/// This pending buffer is global to MpcConnection.
/// Should be protected by a mutex.
struct ReadLoopBuffer {
    pending_subscribe: BTreeMap<RecvId, oneshot::Sender<Bytes>>,
    pending_message: BTreeMap<RecvId, Bytes>,
}

impl ReadLoopBuffer {
    fn new() -> Self {
        ReadLoopBuffer {
            pending_subscribe: BTreeMap::new(),
            pending_message: BTreeMap::new(),
        }
    }
}

/// A buffer for MPC write loop that is global to MpcConnection.
/// Should be protected by a mutex.
///
/// When user send the message, the user will first check if any idle socket is
/// available. If so, send the message directly. Otherwise, the message will be
/// stored to `pending_write_task`.
///
/// When the socket becomes available, it will check if there is any task in
/// `pending_write_task`. If so, remove that write task and run this task.
/// Otherwise, put itself to `pending_idle_socket`.
struct WriteLoopBuffer {
    pending_write_task: VecDeque<(SendId, Bytes, oneshot::Sender<()>)>,
    pending_idle_socket: VecDeque<oneshot::Sender<(SendId, Bytes, oneshot::Sender<()>)>>,
}

impl WriteLoopBuffer {
    fn new() -> Self {
        Self {
            pending_write_task: Default::default(),
            pending_idle_socket: Default::default(),
        }
    }
}

/// Connection abstraction with peer for MPC calculation.
/// Message is sent using load balancing. Each single message will use one
/// socket. Multiple sockets are active when multiple messages are sent.
#[derive(Clone)]
pub struct MpcConnection {
    ip_addr: IpAddr,
    num_bytes_sent: Arc<AtomicUsize>,
    num_bytes_recv: Arc<AtomicUsize>,

    read_loop_buffer: Arc<Mutex<ReadLoopBuffer>>,
    write_loop_buffer: Arc<Mutex<WriteLoopBuffer>>,
}

impl MpcConnection {
    /// Alice listens to the port
    pub async fn new_as_alice(host_port: u16, num_sockets: usize) -> Self {
        let listener = TcpListener::bind(("0.0.0.0", host_port)).await.unwrap();

        info!("Listening to {}", host_port);
        let mut sockets = Vec::with_capacity(num_sockets);
        for _ in 0..num_sockets {
            let (socket, _) = listener.accept().await.unwrap();

            sockets.push(socket);
        }
        let remote_addr = sockets[0].peer_addr().unwrap().ip();

        info!("connection established: {}", remote_addr);
        Self::from_sockets(sockets)
    }

    /// Bob connects to the port
    pub async fn new_as_bob(
        alice_addr: impl ToSocketAddrs + Copy + Debug,
        num_sockets: usize,
    ) -> Self {
        let mut sockets = Vec::with_capacity(num_sockets);
        for _ in 0..num_sockets {
            let socket = tcp_connect_or_retry(alice_addr).await;
            sockets.push(socket);
        }
        let remote_addr = sockets[0].peer_addr().unwrap().ip();

        info!("connection established: {}", remote_addr);
        Self::from_sockets(sockets)
    }

    pub fn dummy() -> Self {
        Self{
            num_bytes_sent: Arc::new(AtomicUsize::new(0)),
            num_bytes_recv: Arc::new(AtomicUsize::new(0)),
            ip_addr: IpAddr::from_str("0.0.0.0").unwrap(),
            read_loop_buffer: Arc::new(Mutex::new(ReadLoopBuffer::new())),
            write_loop_buffer: Arc::new(Mutex::new(WriteLoopBuffer::new())),
        }
    }

    fn from_sockets(sockets: Vec<TcpStream>) -> Self {
        let ip_addr = sockets[0].peer_addr().unwrap().ip();
        // split each socket
        let (read_sockets, write_sockets): (Vec<_>, Vec<_>) = sockets
            .into_iter()
            .map(|socket| socket.into_split())
            .unzip();

        let read_loop_buffer = Arc::new(Mutex::new(ReadLoopBuffer::new()));
        let write_loop_buffer = Arc::new(Mutex::new(WriteLoopBuffer::new()));
        let num_bytes_sent = Arc::new(AtomicUsize::new(0));
        let num_bytes_recv = Arc::new(AtomicUsize::new(0));

        // read loop
        for (idx, socket) in read_sockets.into_iter().enumerate() {
            let pending_buffer = read_loop_buffer.clone();
            let num_bytes_sent = num_bytes_sent.clone();
            tokio::spawn(async move {
                let mut read_socket = BufReader::with_capacity(MPC_TCP_BUFFER_SIZE, socket);
                loop {
                    let (message_id, read_buffer) = match read_one_message(&mut read_socket).await {
                        Ok(message) => message,
                        Err(e) => {
                            debug!("read_one_message error: {:?}", e);
                            break;
                        },
                    };
                    let read_buffer_len = read_buffer.len();
                    num_bytes_sent.fetch_add(read_buffer_len, Ordering::Relaxed);
                    {
                        let mut pending = pending_buffer.lock().unwrap();
                        // if there is pending subscribe, send the message to pending subscribe
                        // channel
                        if let Some(v) = pending.pending_subscribe.remove(&message_id) {
                            if let Err(_) = v.send(read_buffer) {
                                debug!("subscribe reader is dead")
                            };
                            debug!(
                                "{}: done read buffer of size: {}, id: {}, satisfy to pending subscribe",
                                idx,
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

        // write loop
        for write_socket in write_sockets {
            let pending_buffer = write_loop_buffer.clone();
            let num_bytes_recv = num_bytes_recv.clone();
            tokio::spawn(async move {
                let mut write_socket = BufWriter::with_capacity(MPC_TCP_BUFFER_SIZE, write_socket);
                loop {
                    let msg_to_write = {
                        let mut pending = pending_buffer.lock().unwrap();
                        if let Some((send_id, msg, complete)) =
                            pending.pending_write_task.pop_front()
                        {
                            trace!("found a write task: id: {}, length: {}", send_id, msg.len());
                            Upcoming::Ready((send_id, msg, complete))
                        } else {
                            let mut pending = pending;
                            let (tx, rx) = oneshot::channel();
                            pending.pending_idle_socket.push_back(tx);
                            Upcoming::Wait(rx)
                        }
                    };

                    let (message_id, data, complete) = match msg_to_write {
                        Upcoming::Ready(v) => v,
                        Upcoming::Wait(rx) => {
                            // Since the send queue is empty, I can flush the socket
                            write_socket.flush().await.unwrap();
                            rx.await.unwrap()
                        },
                    };

                    let data_len = data.len();

                    // no need to flush because there may be more data to write
                    write_one_message_without_flush(&mut write_socket, message_id, data)
                        .await
                        .unwrap();

                    complete.send(()).unwrap_or_else(|_| {});

                    num_bytes_recv.fetch_add(data_len, Ordering::Relaxed);
                }
            });
        }

        Self {
            ip_addr,
            num_bytes_sent,
            num_bytes_recv,
            read_loop_buffer,
            write_loop_buffer,
        }
    }
}

impl MpcConnection {
    pub fn ip_addr(&self) -> IpAddr {
        self.ip_addr
    }

    pub fn num_bytes_received(&self) -> usize {
        self.num_bytes_recv.load(Ordering::Relaxed)
    }

    pub fn num_bytes_sent(&self) -> usize {
        self.num_bytes_sent.load(Ordering::Relaxed)
    }

    pub fn send_message_bytes(&self, id: SendId, message: Bytes) -> oneshot::Receiver<()> {
        let mut pending = self.write_loop_buffer.lock().unwrap();
        let (s, r) = oneshot::channel();
        if let Some(idle_socket) = pending.pending_idle_socket.pop_front() {
            idle_socket.send((id, message, s)).unwrap();
        } else {
            // otherwise, just append this message to pending write task
            pending.pending_write_task.push_back((id, message, s));
        }
        r
    }

    pub async fn subscribe_and_get_bytes(&self, message_id: RecvId) -> Result<Bytes> {
        let val = {
            let mut pending = self.read_loop_buffer.lock().unwrap();
            if let Some(v) = pending.pending_message.remove(&message_id) {
                trace!("found subscribed data: id={:?}", message_id);
                Upcoming::Ready(v)
            } else {
                // create a one-shot channel
                let (sender, receiver) = oneshot::channel();
                // if there is not: add them to pending subscription
                trace!(
                    "not found subscribed data: id={}, put to pending subscribe",
                    message_id.0
                );
                if pending
                    .pending_subscribe
                    .insert(message_id, sender)
                    .is_some()
                {
                    panic!("duplicate id got subscribed: {:?}", message_id);
                };
                Upcoming::Wait(receiver)
            }
        };
        match val {
            Upcoming::Ready(v) => Ok(v),
            Upcoming::Wait(v) => Ok(v.await.unwrap_or_else(|_| panic!("id={}", message_id.0))),
        }
    }

    pub fn send_message<M: Communicate>(&self, id: SendId, msg: M) -> oneshot::Receiver<()> {
        let data = msg.into_bytes_owned();
        self.send_message_bytes(id, data)
    }

    pub fn send_message_dummy<M: Communicate>(&self, _id: SendId, msg: M) -> oneshot::Receiver<()> {
        msg.drop_into_black_box();
        let (s, r) = oneshot::channel();
        s.send(()).unwrap();
        r
    }

    pub async fn subscribe_and_get<M: Communicate>(&self, id: RecvId) -> Result<M::Deserialized> {
        let data = self.subscribe_and_get_bytes(id).await?;
        Ok(M::from_bytes_owned(data)?)
    }

    pub async fn exchange_message<M: Communicate>(
        &self,
        id: ExchangeId,
        msg: M,
    ) -> Result<M::Deserialized> {
        let send_handle = self.send_message(id.send_id, msg);
        let result = self.subscribe_and_get::<M>(id.recv_id).await;
        send_handle.await.unwrap();
        result
    }
}

pub async fn mpc_localhost_pair(
    host_port: u16,
    num_sockets: usize,
) -> (MpcConnection, MpcConnection) {
    let alice_handle =
        tokio::spawn(async move { MpcConnection::new_as_alice(host_port, num_sockets).await });

    let guest_handle = tokio::spawn(async move {
        MpcConnection::new_as_bob(("localhost", host_port), num_sockets).await
    });

    (
        alice_handle.await.expect("host panic"),
        guest_handle.await.expect("guest panic"),
    )
}

#[cfg(test)]
mod tests {
    use std::time;

    use bytes::Bytes;

    use crate::mpc_conn::mpc_localhost_pair;

    const TEST_PORT: u16 = 6665;

    #[tokio::test]
    #[ignore]
    async fn test_exchange_small() {
        const NUM_CONN: usize = 16;

        let msg1 = vec![11u32, 22, 33, 44];
        let msg2 = vec![55u32, 66, 77, 88];

        let expected1 = msg1.clone();
        let expected2 = msg2.clone();

        let (server1, server2) = mpc_localhost_pair(TEST_PORT, NUM_CONN).await;
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
    async fn test_exchange_medium() {
        const NUM_CONN: usize = 16;

        let msg1 = vec![11u32; 10000];
        let msg2 = vec![55u32; 10000];

        let expected1 = msg1.clone();
        let expected2 = msg2.clone();

        let (server1, server2) = mpc_localhost_pair(TEST_PORT, NUM_CONN).await;
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

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_exchange_bench() {
        const NUM_BYTES: usize = 500000000;
        let msg1: Bytes = vec![1u8; NUM_BYTES].into();
        let msg2: Bytes = vec![2u8; NUM_BYTES].into();

        let (server1, server2) = mpc_localhost_pair(TEST_PORT, 2).await;
        let server1_handle = tokio::spawn(async move {
            let received1 = server1.exchange_message(12.into(), msg1).await.unwrap();
            (received1, server1)
        });

        let server2_handle = tokio::spawn(async move {
            let received2 = server2.exchange_message(12.into(), msg2).await.unwrap();
            (received2, server2)
        });

        let t0 = time::Instant::now();
        let (..) = server1_handle.await.unwrap();
        let t1 = time::Instant::now();
        let (..) = server2_handle.await.unwrap();
        let t2 = time::Instant::now();

        let speed = (NUM_BYTES) as f64 / (t1 - t0).as_secs_f64() / (1 << 20) as f64;

        println!(
            "Exchange speed: {} MB/s. Time taken: {}s, extra_time: {}s",
            speed,
            (t1 - t0).as_secs_f64(),
            (t2 - t1).as_secs_f64()
        );
    }
}
