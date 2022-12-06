use bridge::{
    client_server::ClientsPool, end_timer, id_tracker::IdGen, mpc_conn::MpcConnection, start_timer,
};

use prio::{encrypt::*, field::*, server::*};

use bridge::id_tracker::RecvId;
use crypto_primitives::uint::UInt;
use rayon::prelude::*;
use serialize::UseSerde;
use tracing::info;

pub struct Statistics {
    pub client_comm: usize,
    pub client_time: f64,
    pub mpc_comm: usize,
    pub mpc_prepare: f64,
    pub mpc_verify: f64,
}

/// Basic version of the FL server.
/// server id is 0 if b is false, otherwise it is 1.
pub async fn basic_server<I, F>(
    is_bob: bool,
    clients: &ClientsPool,
    gsize: usize,
    batch_size: usize,
    peer: MpcConnection,
    eval_at: F,
) -> (
    Vec<VerificationMessage<F>>,
    Vec<VerificationMessage<F>>,
    Statistics,
)
where
    I: UInt,
    F: FieldElement + Send + Sync,
{
    let mut id = IdGen::new();
    // track the message id with client, and message id with peer
    let dim = I::NUM_BITS * gsize;

    let timer = start_timer!(|| "Receive Input Sharing and SNIP Proof from Client");
    // Stage 1: Receiving Input Shares and SNIP from Clients
    // For each client, the basic server will receive the boolean share of
    // input. The ClientsPool will group all data together.

    let client_messages = clients
        .subscribe_and_get_bytes(RecvId::FIRST)
        .await
        .unwrap();

    println!(
        "Client messages size: {}x({}, {})",
        client_messages.len(),
        client_messages[0].len(),
        client_messages[1].len()
    );
    let client_time = end_timer!(timer).elapsed().as_secs_f64();

    let timer = start_timer!(|| "Server prepare verification messages");

    // Warning: Do not hardcode this in production case. This is only for testing.
    // Each server should have unique pair of alice and bob private key, but here they are using the same, just
    // for code simplicity.
    let bob_priv_key = PrivateKey::from_base64(
        "BIl6j+J6dYttxALdjISDv6ZI4/VWVEhUzaS05LgrsfswmbLOgN\
             t9HUC2E0w+9RqZx3XMkdEHBHfNuCSMpOwofVSq3TfyKwn0NrftKisKKVSaTOt5seJ67P5QL4hxgPWvxw==",
    )
    .unwrap();
    let alice_priv_key = PrivateKey::from_base64(
        "BNNOqoU54GPo+1gTPv+hCgA9U2ZCKd76yOMrWa1xTWgeb4LhF\
             LMQIQoRwDVaW64g/WTdcxT4rDULoycUNFB60LER6hPEHg/ObBnRPV1rwS3nj9Bj0tbjVPPyL9p8QW8B+w==",
    )
    .unwrap();

    let (mut msgs_as_alice, mut msgs_as_bob) = (Vec::new(), Vec::new());
    for (idx, msg) in client_messages.into_iter().enumerate() {
        match (idx % 2 == 0, is_bob) {
            (true, false) => msgs_as_alice.push(msg),
            (true, true) => msgs_as_bob.push(msg),
            (false, false) => msgs_as_bob.push(msg),
            (false, true) => msgs_as_alice.push(msg),
        }
    }

    info!("msgs_as_alice length: {}", msgs_as_alice[0].len());
    info!("msgs_as_bob length: {}", msgs_as_bob[0].len());
    info!("using batch size: {}", batch_size);

    let local_verif_messages_as_alice = msgs_as_alice
        .chunks(batch_size)
        .map(|chunk| {
            chunk
                .par_iter()
                .map(|msg| {
                    let mut sv = Server::new(dim, true, alice_priv_key.clone()).unwrap();
                    sv.generate_verification_message(eval_at, &msg[..]).unwrap()
                })
                .collect::<Vec<_>>()
        })
        .flatten();

    let local_verif_messages_as_bob = msgs_as_bob
        .chunks(batch_size)
        .map(|chunk| {
            chunk
                .par_iter()
                .map(|msg| {
                    let mut sv = Server::new(dim, false, bob_priv_key.clone()).unwrap();
                    sv.generate_verification_message(eval_at, &msg[..]).unwrap()
                })
                .collect::<Vec<_>>()
        })
        .flatten();

    let local_verif_messages = local_verif_messages_as_alice
        .chain(local_verif_messages_as_bob)
        .collect::<Vec<_>>();

    let mpc_prepare = end_timer!(timer).elapsed().as_secs_f64();

    let timer = start_timer!(|| "Server Exchange verification messages");
    let peer_verif_messages = peer
        .exchange_message(
            id.next_exchange_id(),
            &UseSerde(
                local_verif_messages
                    .iter()
                    .map(|x| (x.f_r, x.g_r, x.h_r))
                    .collect::<Vec<_>>(),
            ),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|(f_r, g_r, h_r)| VerificationMessage { f_r, g_r, h_r })
        .collect::<Vec<_>>();

    let mpc_verify = end_timer!(timer).elapsed().as_secs_f64();

    (
        local_verif_messages,
        peer_verif_messages,
        Statistics {
            client_comm: clients.num_bytes_received_from_all(),
            client_time,
            mpc_comm: peer.num_bytes_received(),
            mpc_prepare,
            mpc_verify,
        },
    )
}
