//! Client interaction
use bridge::{
    client_server::ClientsPool,
    end_timer,
    id_tracker::{RecvId, SendId},
    start_timer,
};
use crypto_primitives::{
    malpriv::MessageHash,
    message::po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    uint::UInt,
};
use serialize::AsUseCast;
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct ClientData<I: UInt, H: MessageHash> {
    pub po2_msgs_alice: Arc<[ClientPo2MsgToAlice]>,
    pub po2_msgs_bob: Arc<[ClientPo2MsgToBob<I>]>,

    pub comm_alice: usize,
    pub comm_bob: usize,

    pub phase1_time: f64,
    /// B2A hashes from Alice to Bob, for clients where I'm Bob
    pub hash_b2a_ab: Vec<H::Output>,

    pub phase2_time: f64,
    /// OT verification hashes from Bob to Alice, for clients where I'm Alice
    pub hash_ot_ba: Vec<H::Output>,
}

impl<I: UInt, H: MessageHash> ClientData<I, H> {
    pub fn num_clients_as_alice(&self) -> usize {
        self.po2_msgs_alice.len()
    }

    pub fn num_clients_as_bob(&self) -> usize {
        self.po2_msgs_bob.len()
    }

    pub async fn fetch(is_alice: bool, port: u16, num_clients: usize, chi_seed: u64) -> Self {
        let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
        // accepts clients connection
        let clients = ClientsPool::new(num_clients, listener).await;
        // load balancing: split the clients pool and ALICE pool and BOB pool, notice
        // that this "Bob" is different from the "bob"
        // for global server role.  Alice is OT sender, Bob is OT receiver.
        let (clients_alice, clients_bob) = clients.split(is_alice);

        let timer = start_timer!(|| "Client Phase 1");

        let alice_msg = {
            let clients_alice = clients_alice.clone();
            tokio::spawn(async move {
                clients_alice
                    .subscribe_and_get::<ClientPo2MsgToAlice>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let bob_msg = {
            let clients_bob = clients_bob.clone();
            tokio::spawn(async move {
                clients_bob
                    .subscribe_and_get::<(ClientPo2MsgToBob<I>, H::Output)>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let (alice_msg, bob_msg) = tokio::join!(alice_msg, bob_msg);
        let (alice_msg, bob_msg) = (alice_msg.unwrap(), bob_msg.unwrap());

        let po2_msgs_alice = Arc::<[_]>::from(alice_msg.into_boxed_slice());

        let mut po2_msgs_bob = Vec::with_capacity(bob_msg.len());
        let mut hash_b2a_ab = Vec::with_capacity(bob_msg.len());

        for (m, h_b2a) in bob_msg {
            po2_msgs_bob.push(m);
            hash_b2a_ab.push(h_b2a);
        }

        let po2_msgs_bob = Arc::<[_]>::from(po2_msgs_bob);

        let phase1_time = end_timer!(timer).elapsed().as_secs_f64();

        let timer = start_timer!(|| "Client Phase 2");
        // broadcast alice client `chi_seed` and `t_seed`
        clients_alice
            .broadcast_messages(SendId::FIRST, chi_seed.use_cast())
            .await;

        // receive phase 2 hashes for both alice and bob
        let hash_ot_ba = {
            let clients_alice = clients_alice.clone();
            clients_alice
                .subscribe_and_get::<H::Output>(RecvId::SECOND)
                .await
                .unwrap()
        };

        let phase2_time = end_timer!(timer).elapsed().as_secs_f64();

        let comm_alice = clients_alice.num_bytes_received_from_all();
        let comm_bob = clients_bob.num_bytes_received_from_all();
        Self {
            po2_msgs_alice,
            po2_msgs_bob,
            comm_alice,
            comm_bob,
            phase1_time,
            phase2_time,
            hash_b2a_ab,
            hash_ot_ba,
        }
    }
}
