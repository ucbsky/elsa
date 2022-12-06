//! Client interaction
use bridge::{client_server::ClientsPool, end_timer, id_tracker::RecvId, start_timer};
use crypto_primitives::{
    malpriv::MessageHash,
    message::{
        l2::{ClientMPMsgToAlice, ClientMPMsgToBob},
        po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    },
    square_corr::SquareCorrShare,
    uint::UInt,
    utils::bytes_to_seed_pairs,
};
use rayon::prelude::*;

use std::sync::Arc;
use tokio::net::TcpListener;

pub struct ClientData<I: UInt, C: UInt, H: MessageHash> {
    pub po2_msgs_alice: Arc<[ClientPo2MsgToAlice]>,
    pub po2_msgs_bob: Arc<[ClientPo2MsgToBob<I>]>,

    pub sqcorr_alice: Arc<[Vec<SquareCorrShare<C>>]>,
    pub sqcorr_bob: Arc<[Vec<SquareCorrShare<C>>]>,

    pub comm_alice: usize,
    pub comm_bob: usize,

    pub phase1_time: f64,
    /// B2A hashes from Alice to Bob, for clients where I'm Bob
    pub hash_b2a_ab: Vec<H::Output>,
    /// A2S hashes for messages from peer
    pub hash_a2s: Vec<H::Output>,

    pub phase2_time: f64,
    /// OT verification hashes from Bob to Alice, for clients where I'm Alice
    pub hash_ot_ba: Vec<H::Output>,
    /// Square-correlation verification hashes for messages from peer
    pub hash_sqcorr_ab: Vec<H::Output>,
    pub hash_sqcorr_ba: Vec<H::Output>,

    pub chi_seed_share: Vec<u64>,
    pub t_seed_share: Vec<u64>,
}

impl<I: UInt, C: UInt, H: MessageHash<Output = Vec<u8>>> ClientData<I, C, H> {
    pub fn num_clients_as_alice(&self) -> usize {
        self.po2_msgs_alice.len()
    }

    pub fn num_clients_as_bob(&self) -> usize {
        self.po2_msgs_bob.len()
    }

    pub fn num_clients(&self) -> usize {
        self.num_clients_as_alice() + self.num_clients_as_bob()
    }

    pub async fn fetch<F>(
        is_alice: bool,
        port: u16,
        num_clients: usize,
        gsize: usize,
        hasher: F,
    ) -> Self
    where
        F: Fn() -> H + Sync,
    {
        let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
        // accepts clients connection
        let clients = ClientsPool::new(num_clients, listener).await;
        // load balancing: split the clients pool and ALICE pool and BOB pool, notice
        // that this "Bob" is different from the "bob"
        // for global server role.  Alice is OT sender, Bob is OT receiver.
        let (clients_alice, clients_bob) = clients.split(is_alice);

        let timer = start_timer!(|| "Client Fetch");

        let alice_msg = {
            let clients_alice = clients_alice.clone();
            tokio::spawn(async move {
                clients_alice
                    .subscribe_and_get::<ClientMPMsgToAlice<H>>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let bob_msg = {
            let clients_bob = clients_bob.clone();
            tokio::spawn(async move {
                clients_bob
                    .subscribe_and_get::<ClientMPMsgToBob<I, C, H>>(RecvId::FIRST)
                    .await
                    .unwrap()
            })
        };
        let (alice_msg, bob_msg) = tokio::join!(alice_msg, bob_msg);
        let (alice_msg, bob_msg) = (alice_msg.unwrap(), bob_msg.unwrap());

        let (chi_seeds_a, t_seeds_a) = alice_msg
            .par_iter()
            .map(|(phase_1_msg, _)| {
                let mut hasher = hasher();
                hasher.absorb(&phase_1_msg);
                let hash = hasher.digest();
                bytes_to_seed_pairs(&hash)
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let (chi_seeds_b, t_seeds_b) = bob_msg
            .par_iter()
            .map(|(phase_1_msg, _)| {
                let mut hasher = hasher();
                hasher.absorb(&phase_1_msg);
                let hash = hasher.digest();
                bytes_to_seed_pairs(&hash)
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let mut po2_msgs_alice = Vec::with_capacity(alice_msg.len());
        let mut sqcorr_alice = Vec::with_capacity(alice_msg.len());
        let mut hash_a2s_ba = Vec::with_capacity(alice_msg.len());

        let mut hash_ot_ba = Vec::with_capacity(alice_msg.len());
        let mut hash_sqcorr_ab = Vec::with_capacity(bob_msg.len());
        let mut hash_sqcorr_ba = Vec::with_capacity(alice_msg.len());

        for ((m, h), (h_ot_ba, h_sqcorr_ba)) in alice_msg {
            po2_msgs_alice.push(m.po2_msg);
            sqcorr_alice.push(m.square_corr);
            hash_a2s_ba.push(h);
            hash_ot_ba.push(h_ot_ba);
            hash_sqcorr_ba.push(h_sqcorr_ba);
        }

        let sqcorr_alice = sqcorr_alice
            .into_par_iter()
            .map(|v| v.expand(gsize * 2))
            .collect::<Vec<_>>();
        let sqcorr_alice = Arc::<[_]>::from(sqcorr_alice);

        let po2_msgs_alice = Arc::<[_]>::from(po2_msgs_alice.into_boxed_slice());

        let mut po2_msgs_bob = Vec::with_capacity(bob_msg.len());
        let mut sqcorr_bob = Vec::with_capacity(bob_msg.len());
        let mut hash_b2a_ab = Vec::with_capacity(bob_msg.len());
        let mut hash_a2s_ab = Vec::with_capacity(bob_msg.len());
        for ((m, h_b2a, h_a2s), h_sqcorr_ab) in bob_msg {
            po2_msgs_bob.push(m.po2_msg);
            sqcorr_bob.push(m.square_corr);
            hash_b2a_ab.push(h_b2a);
            hash_a2s_ab.push(h_a2s);
            hash_sqcorr_ab.push(h_sqcorr_ab);
        }
        let sqcorr_bob = sqcorr_bob
            .into_par_iter()
            .map(|v| v.expand())
            .collect::<Vec<_>>();
        let sqcorr_bob = Arc::<[_]>::from(sqcorr_bob);

        let po2_msgs_bob = Arc::<[_]>::from(po2_msgs_bob);

        let hash_a2s =
            ClientsPool::merge_msg(is_alice, hash_a2s_ba.into_iter(), hash_a2s_ab.into_iter());
        let chi_seed_share =
            ClientsPool::merge_msg(is_alice, chi_seeds_a.into_iter(), chi_seeds_b.into_iter());
        let t_seed_share =
            ClientsPool::merge_msg(is_alice, t_seeds_a.into_iter(), t_seeds_b.into_iter());

        let phase1_time = end_timer!(timer).elapsed().as_secs_f64();

        let comm_alice = clients_alice.num_bytes_received_from_all();
        let comm_bob = clients_bob.num_bytes_received_from_all();
        Self {
            po2_msgs_alice,
            po2_msgs_bob,
            sqcorr_alice,
            sqcorr_bob,
            comm_alice,
            comm_bob,
            phase1_time,
            phase2_time: 0.,
            hash_b2a_ab,
            hash_a2s,
            hash_ot_ba,
            hash_sqcorr_ab,
            hash_sqcorr_ba,
            chi_seed_share,
            t_seed_share,
        }
    }
}
