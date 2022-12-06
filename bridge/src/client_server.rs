use std::{collections::BTreeSet, fmt::Debug, iter::FromIterator};

use bytes::Bytes;
use tokio::net::{TcpListener, ToSocketAddrs};
use tracing::{debug, error};

use itertools::Itertools;
use serialize::Communicate;

use crate::{
    id_tracker::{RecvId, SendId},
    tcp_bridge::{ClientID, TcpConnection},
    tcp_connect_or_retry,
};

type Error = crate::BridgeError;
type Result<T> = std::result::Result<T, Error>;

/// An agent that receive data with multiple clients.
#[derive(Clone)]
pub struct ClientsPool {
    pub clients: Vec<TcpConnection>,
}

impl ClientsPool {
    pub async fn new(num_clients: usize, listener: TcpListener) -> Self {
        // first, accept all the needed clients
        let mut clients_handle = Vec::with_capacity(num_clients);
        for _ in 0..num_clients {
            let (socket, addr) = listener.accept().await.unwrap();
            debug!("Connected to peer at {}", addr);
            let conn = tokio::spawn(TcpConnection::new_server_side(socket));
            clients_handle.push(conn);
        }
        let mut clients = Vec::with_capacity(num_clients);
        for c in clients_handle {
            clients.push(c.await.unwrap());
        }
        clients.sort_by_key(|c| c.uid());

        // check if there is any duplicate key
        assert_eq!(
            clients
                .iter()
                .map(|x| x.uid())
                .collect::<BTreeSet<_>>()
                .len(),
            clients.len(),
            "Duplicate client uid"
        );
        Self { clients }
    }

    pub fn num_of_clients(&self) -> usize {
        self.clients.len()
    }

    pub fn num_bytes_received_from_all(&self) -> usize {
        self.clients
            .iter()
            .map(|client| client.num_bytes_received())
            .sum()
    }

    /// subscribe and wait to get bytes
    pub async fn subscribe_and_get_bytes(&self, message_id: RecvId) -> Result<Vec<Bytes>> {
        // for each client, subscribe the struct
        let msg_handle = self
            .clients
            .iter()
            .map(|client| {
                let client = client.clone();
                tokio::spawn(async move { client.subscribe_and_get_bytes(message_id).await })
            })
            .collect::<Vec<_>>();
        let mut result = Vec::with_capacity(self.clients.len());
        for handle in msg_handle {
            result.push(handle.await.unwrap());
        }

        return Ok(result);
    }

    /// Subscribe and get message that does not contain any references
    pub async fn subscribe_and_get<T: Communicate>(
        &self,
        message_id: RecvId,
    ) -> Result<Vec<T::Deserialized>> {
        // for each client, subscribe the struct
        let msg_handle = self
            .clients
            .iter()
            .map(|client| {
                let client = client.clone();
                tokio::spawn(
                    async move { client.subscribe_and_get::<T>(message_id).await.unwrap() },
                )
            })
            .collect::<Vec<_>>();
        let mut result = Vec::with_capacity(self.clients.len());
        for handle in msg_handle {
            result.push(handle.await.unwrap());
        }

        return Ok(result);
    }

    /// Broadcast message as bytes to all clients
    pub async fn broadcast_messages_as_bytes(&self, message_id: SendId, message: Bytes) {
        let handles = self
            .clients
            .iter()
            .map(|client| {
                let message = message.clone(); // this is cheap
                let client = client.clone();
                tokio::spawn(async move { client.send_message_bytes(message_id, message) })
            })
            .collect::<Vec<_>>();

        // wait for all handles to complete
        for handle in handles {
            match handle.await {
                Ok(w) => w.await.unwrap_or_else(|e| {
                    error!("failed to send message: {:?}", e);
                }),
                Err(e) => {
                    error!("failed to send message: {:?}", e);
                },
            }
        }
    }

    pub async fn broadcast_messages<T: Communicate>(&self, message_id: SendId, message: T) {
        let message_bytes = message.into_bytes_owned();
        self.broadcast_messages_as_bytes(message_id, message_bytes)
            .await;
    }

    pub fn iter(&self) -> impl Iterator<Item = &TcpConnection> {
        self.clients.iter()
    }

    pub fn split(&self, is_alice: bool) -> (Self, Self) {
        let clients_with_odd_uid = self
            .iter()
            .filter(|c| c.uid().is_odd())
            .cloned()
            .collect::<ClientsPool>();
        let clients_with_even_uid = self
            .iter()
            .filter(|c| c.uid().is_even())
            .cloned()
            .collect::<ClientsPool>();
        let (clients_alice, clients_bob) = if is_alice {
            (clients_with_even_uid, clients_with_odd_uid)
        } else {
            (clients_with_odd_uid, clients_with_even_uid)
        };
        (clients_alice, clients_bob)
    }

    pub fn merge_msg<'a, T>(
        is_alice: bool,
        from_alice: impl Iterator<Item = T> + 'a,
        from_bob: impl Iterator<Item = T> + 'a,
    ) -> Vec<T> {
        if is_alice {
            // from_alice is even uid
            from_alice.interleave(from_bob).collect()
        } else {
            from_bob.interleave(from_alice).collect()
        }
    }

    pub fn split_iter<'a, T>(
        is_alice: bool,
        msg: impl Iterator<Item = T> + 'a,
    ) -> (Vec<T>, Vec<T>) {
        let mut alice = Vec::new();
        let mut bob = Vec::new();
        let mut i = 0usize;
        for m in msg {
            if is_alice {
                if i % 2 == 0 {
                    alice.push(m);
                } else {
                    bob.push(m);
                }
            } else {
                if i % 2 == 0 {
                    bob.push(m);
                } else {
                    alice.push(m);
                }
            }
            i += 1;
        }
        (alice, bob)
    }
}

impl FromIterator<TcpConnection> for ClientsPool {
    fn from_iter<T: IntoIterator<Item = TcpConnection>>(iter: T) -> Self {
        Self {
            clients: iter.into_iter().collect(),
        }
    }
}

/// returns a vector of length `num_of_clients` with each element a pair of
/// (address_to_server0, address_to_server1)
pub async fn init_meta_clients(
    num_clients: usize,
    server0: impl ToSocketAddrs + Copy + Debug,
    server1: impl ToSocketAddrs + Copy + Debug,
) -> Vec<(TcpConnection, TcpConnection)> {
    let mut connections = Vec::with_capacity(num_clients);
    let mut progresses = Vec::with_capacity(num_clients * 2);
    for uid in 0..num_clients {
        let uid = ClientID::new(uid as u64);
        let socket0 = tcp_connect_or_retry(server0).await;
        let socket1 = tcp_connect_or_retry(server1).await;
        debug!(
            "Connected to peer at server0 at {}",
            socket0.peer_addr().unwrap()
        );
        debug!(
            "Connected to peer at server1 at {}",
            socket1.peer_addr().unwrap()
        );
        let (conn0, p0) = TcpConnection::new_client_side(socket0, uid);
        let (conn1, p1) = TcpConnection::new_client_side(socket1, uid);
        connections.push((conn0, conn1));
        progresses.push(p0);
        progresses.push(p1);
    }
    for w in progresses {
        w.await.unwrap();
    }

    connections
}

#[cfg(test)]
mod tests {
    use tokio::net::{TcpListener, TcpStream};
    use tracing::{info, Level};

    use serialize::UseCast;

    use crate::{
        client_server::ClientsPool,
        tcp_bridge::{ClientID, TcpConnection},
    };

    const TEST_ADDRESS: &str = "localhost:6665";

    const NUM_CLIENTS: usize = 8;

    #[tokio::test]
    #[ignore]
    async fn test_aggregator() {
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::INFO)
            .init();
        let handle = tokio::spawn(async move {
            let listener = TcpListener::bind(TEST_ADDRESS).await.unwrap();
            info!("Listening to {}", TEST_ADDRESS);

            let aggregator = ClientsPool::new(NUM_CLIENTS, listener).await;

            let received_payload = aggregator
                .subscribe_and_get::<UseCast<usize>>(12.into())
                .await
                .unwrap();

            assert_eq!(received_payload, (0..NUM_CLIENTS).collect::<Vec<_>>());

            info!("Received Payload: {:?}", received_payload);
        });

        (0..NUM_CLIENTS).for_each(|client_index| {
            tokio::spawn(async move {
                let socket;
                loop {
                    match TcpStream::connect(TEST_ADDRESS).await {
                        Ok(s) => {
                            socket = s;
                            break;
                        },
                        Err(_) => {
                            info!("waiting to connecting in 10ms");
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        },
                    }
                }
                let (conn, wait) =
                    TcpConnection::new_client_side(socket, ClientID::new(client_index as u64));
                wait.await.unwrap();
                conn.send_message(12.into(), &UseCast(client_index))
                    .unwrap();
            });
        });

        handle.await.unwrap();
    }
}
