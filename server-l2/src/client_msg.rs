//! Client interaction
use bridge::{client_server::ClientsPool, end_timer, id_tracker::RecvId, start_timer};
use crypto_primitives::{
    message::{
        l2::{ClientL2MsgToAlice, ClientL2MsgToBob},
        po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    },
    square_corr::SquareCorrShare,
    uint::UInt,
};
use rayon::prelude::*;
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct ClientData<I: UInt, C: UInt> {
    pub po2_msgs_alice: Arc<[ClientPo2MsgToAlice]>,
    pub po2_msgs_bob: Arc<[ClientPo2MsgToBob<I>]>,

    pub sqcorr: Arc<[Vec<SquareCorrShare<C>>]>,

    pub comm_alice: usize,
    pub comm_bob: usize,

    pub time: f64,
}

impl<I: UInt, C: UInt> ClientData<I, C> {
    pub fn num_clients_as_alice(&self) -> usize {
        self.po2_msgs_alice.len()
    }

    pub fn num_clients_as_bob(&self) -> usize {
        self.po2_msgs_bob.len()
    }

    pub fn num_clients(&self) -> usize {
        self.num_clients_as_alice() + self.num_clients_as_bob()
    }

    pub async fn fetch(is_alice: bool, port: u16, num_clients: usize, gsize: usize) -> Self {
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
                    .subscribe_and_get::<ClientL2MsgToAlice>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let bob_msg = {
            let clients_bob = clients_bob.clone();
            tokio::spawn(async move {
                clients_bob
                    .subscribe_and_get::<ClientL2MsgToBob<I, C>>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let (alice_msg, bob_msg) = tokio::join!(alice_msg, bob_msg);
        let (alice_msg, bob_msg) = (alice_msg.unwrap(), bob_msg.unwrap());

        let mut po2_msgs_alice = Vec::with_capacity(alice_msg.len());
        let mut sqcorr_alice = Vec::with_capacity(alice_msg.len());

        for m in alice_msg {
            po2_msgs_alice.push(m.po2_msg);
            sqcorr_alice.push(m.square_corr);
        }

        let sqcorr_alice = sqcorr_alice
            .into_par_iter()
            .map(|v| v.expand(gsize * 2))
            .collect::<Vec<_>>();

        let po2_msgs_alice = Arc::<[_]>::from(po2_msgs_alice.into_boxed_slice());

        let mut po2_msgs_bob = Vec::with_capacity(bob_msg.len());
        let mut sqcorr_bob = Vec::with_capacity(bob_msg.len());
        for m in bob_msg {
            po2_msgs_bob.push(m.po2_msg);
            sqcorr_bob.push(m.square_corr);
        }
        let sqcorr_bob = sqcorr_bob
            .into_par_iter()
            .map(|v| v.expand())
            .collect::<Vec<_>>();

        let po2_msgs_bob = Arc::<[_]>::from(po2_msgs_bob);
        let sqcorr = Arc::<_>::from(ClientsPool::merge_msg(
            is_alice,
            sqcorr_alice.into_iter(),
            sqcorr_bob.into_iter(),
        ));
        let time = end_timer!(timer).elapsed().as_secs_f64();

        let comm_alice = clients_alice.num_bytes_received_from_all();
        let comm_bob = clients_bob.num_bytes_received_from_all();
        Self {
            po2_msgs_alice,
            po2_msgs_bob,
            sqcorr,
            comm_alice,
            comm_bob,
            time,
        }
    }
}
