use bridge::{id_tracker::SendId, tcp_bridge::TcpConnection};
use client_po2::protocol::SingleRoundClient;
use crypto_primitives::{
    bits::batch_make_boolean_shares,
    cot::client::{num_additional_ot_needed, B2ACOTToAlice, B2ACOTToBob, COTGen},
    message::l2::{ClientL2MsgToAlice, ClientL2MsgToBob},
    square_corr::batch_make_sqcorr_shares,
    uint::UInt,
};
use rand::Rng;
use tokio::sync::oneshot;

/// Client on input ring `I`, and correlation ring `C`
pub struct L2Client<I: UInt, C: UInt> {
    pub prepared_message_0: ClientL2MsgToAlice,
    pub prepared_message_1: ClientL2MsgToBob<I, C>,
}

impl<I: UInt, C: UInt> SingleRoundClient<I> for L2Client<I, C> {
    fn new<R: Rng>(input: &[I], rng: &mut R) -> Self {
        let gsize = input.len();
        let (input_0, input_1) = batch_make_boolean_shares(rng, input.iter().map(|x| x.bits_le()));
        let delta = COTGen::sample_delta(rng);
        let num_additional_cot = num_additional_ot_needed(gsize * I::NUM_BITS as usize);
        let (cot_s, cot_r) = if cfg!(feature = "no-ot") {
            (B2ACOTToAlice::default(), B2ACOTToBob::default())
        } else {
            COTGen::sample_cots(rng, &input_1, delta, num_additional_cot)
        };

        // generate correlation
        let (corr_0, corr_1, ..) = batch_make_sqcorr_shares(rng, gsize * 2);

        let msg0 = ClientL2MsgToAlice::new(input_0, cot_s, corr_0);
        let msg1 = ClientL2MsgToBob::new(input_1, cot_r, corr_1);

        L2Client {
            prepared_message_0: msg0,
            prepared_message_1: msg1,
        }
    }

    fn send_to_ot_sender(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_0).unwrap()
    }

    fn send_to_ot_receiver(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_1).unwrap()
    }
}
