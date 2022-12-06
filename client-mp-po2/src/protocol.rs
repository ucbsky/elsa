use bridge::{
    id_tracker::{RecvId, SendId},
    tcp_bridge::TcpConnection,
};
use crypto_primitives::{
    bits::batch_make_boolean_shares,
    cot::client::{num_additional_ot_needed, COTGen},
    malpriv::{
        client::{simulate_b2a, simulate_ot_verify},
        MessageHash,
    },
    message::po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    uint::UInt,
};
use rand::Rng;
use serialize::UseCast;
use tokio::sync::oneshot;

/// Client on input ring `I`, and correlation ring `C`
pub struct Client<I: UInt, H: MessageHash> {
    /// Po2 message
    pub prepared_message_a: ClientPo2MsgToAlice,
    /// Po2 message, hash_ab for B2A
    pub prepared_message_b: (ClientPo2MsgToBob<I>, H::Output),
}

impl<I: UInt, H: MessageHash> Client<I, H> {
    pub fn prepare_phase1<A: UInt, R: Rng, F>(input: &[I], rng: &mut R, hasher: F) -> Self
    where
        F: Fn() -> H,
    {
        let mut hasher_b2a_ab = hasher(); // hasher of message sent from alice to bob

        let gsize = input.len();
        let (input_0, input_1) = batch_make_boolean_shares(rng, input.iter().map(|x| x.bits_le()));
        let delta = COTGen::sample_delta(rng);
        let num_additional_cot = num_additional_ot_needed(gsize * I::NUM_BITS as usize);
        let (cot_s, cot_r) = COTGen::sample_cots(rng, &input_1, delta, num_additional_cot);

        let input_0_expanded = input_0.expand(gsize);

        // simulate B2A and A2S and get transcript
        let _ = simulate_b2a::<I, A, H>(
            &input_0_expanded,
            &input_1,
            &cot_s,
            &cot_r,
            &mut hasher_b2a_ab,
        );

        let msg_alice = ClientPo2MsgToAlice::new(input_0, cot_s);
        let msg_bob = ClientPo2MsgToBob::new(input_1, cot_r);
        Client {
            prepared_message_a: msg_alice,
            prepared_message_b: (msg_bob, hasher_b2a_ab.digest()),
        }
    }

    pub fn send_to_alice(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_a).unwrap()
    }

    pub fn send_to_bob(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_b).unwrap()
    }

    /// Receive chi seed and t seed from Alice
    pub async fn phase_2<A: UInt, F>(
        &self,
        alice: TcpConnection,
        alice_id: (RecvId, SendId),
        hasher: F,
    ) where
        F: Fn() -> H,
    {
        let mut hasher_ot_ba = hasher();

        let chi_seed = alice
            .subscribe_and_get::<UseCast<u64>>(alice_id.0)
            .await
            .unwrap();
        // verification
        simulate_ot_verify::<I, A, H>(
            &self.prepared_message_b.0.inputs_1,
            &self.prepared_message_b.0.cot,
            chi_seed,
            &mut hasher_ot_ba,
        );

        let alice_handle = alice
            .send_message(alice_id.1, &hasher_ot_ba.digest())
            .unwrap();

        alice_handle.await.unwrap();
    }
    // no need to receive from bob
}
