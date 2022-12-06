use block::{gf::GF2_256, Block};
use bridge::{
    id_tracker::{RecvId, SendId},
    mpc_conn::MpcConnection,
};
use crypto_primitives::{
    b2a::{bit_comp_as_ot_receiver_batch, bit_comp_as_ot_sender_batch},
    cot::{
        client::B2ACOTToAlice,
        server::{OTReceiver, OTSender},
    },
    message::po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    uint::UInt,
};

use serialize::{AsUseCast, UseCast};
use std::sync::Arc;
use tokio::sync::oneshot;

/// Send Handle is a receive channel indicating if `send` is finished.
pub type SendHandle = oneshot::Receiver<()>;

/// Run OT Verify on one client, assuming I'm OT sender.
/// Return COT (qs), verify result, and client message
/// `I` is Input ring. `A` is Aggregation Ring. `C` is Output ring.
pub async fn ot_verify_alice<I: UInt>(
    msg_id: RecvId,
    cot: &B2ACOTToAlice,
    chi: Arc<Vec<Block>>,
    peer: MpcConnection,
) -> (Vec<Block>, bool) {
    // ROUND 1: verify COT

    // receive x_til and t_til from peer
    let (x_til, t_til) = if cfg!(feature = "no-comm") {
        (Default::default(), Default::default())
    } else {
        peer.subscribe_and_get::<(UseCast<Block>, GF2_256)>(msg_id)
            .await
            .unwrap()
    };

    // verify cot
    let (qs, r) = OTSender::verify_and_get_cot(cot.qs_seed, &chi, cot.delta, x_til, t_til);

    (qs, r)
}

/// Run OT Verify on one client, assuming I'm OT receiver. Return a send handle
/// indicating if send has finished.
pub fn ot_verify_bob<I: UInt>(
    msg_id: SendId,
    client_msg: &ClientPo2MsgToBob<I>,
    peer: &MpcConnection,
    chi: Arc<Vec<Block>>,
    gsize: usize,
) -> SendHandle {
    assert_eq!(client_msg.inputs_1.len(), gsize);

    // ROUND 1: verify COT
    let (x_til, t_til) = OTReceiver::send_x_til_t_til(
        &client_msg.cot.ts,
        &chi,
        &client_msg.inputs_1,
        client_msg.cot.r_seed,
    );
    if cfg!(feature = "no-comm") {
        peer.send_message_dummy(msg_id, (x_til.use_cast(), t_til))
    } else {
        peer.send_message(msg_id, (x_til.use_cast(), t_til))
    }
}

/// Run OT B2A on one client, assuming I'm OT sender.
/// Return COT (qs), and a send handle
pub fn b2a_alice<I: UInt, A: UInt>(
    msg_id: SendId,
    gsize: usize,
    client_msg: &ClientPo2MsgToAlice,
    qs: &[Block],
    peer: &MpcConnection,
) -> (Vec<A>, SendHandle) {
    let num_ot = gsize * I::NUM_BITS as usize;
    let qs = &qs[..num_ot];

    let inputs_0 = client_msg.inputs_0.expand::<I>(gsize);
    let (y0s, us) = bit_comp_as_ot_sender_batch(&inputs_0, client_msg.cot.delta, qs);

    // send us
    let send_handle = if cfg!(feature = "no-comm") {
        peer.send_message_dummy(msg_id, us)
    } else {
        peer.send_message(msg_id, us)
    };

    (y0s, send_handle)
}

pub async fn b2a_bob<I: UInt, A: UInt>(
    msg_id: RecvId,
    client_msg: &ClientPo2MsgToBob<I>,
    peer: MpcConnection,
) -> Vec<A> {
    let gsize = client_msg.inputs_1.len();
    let num_ot = gsize * I::NUM_BITS as usize;
    let ts = &client_msg.cot.ts[..num_ot];

    // receive us
    let us = if cfg!(feature = "no-comm") {
        vec![A::zero(); num_ot]
    } else {
        peer.subscribe_and_get::<Vec<A>>(msg_id).await.unwrap()
    };

    bit_comp_as_ot_receiver_batch(&client_msg.inputs_1, ts, &us)
}
