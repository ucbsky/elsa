use crate::{client_msg::ClientData, utils::IdPool};
use bin_utils::server::{InputSize, Options};
use bridge::{end_timer, mpc_conn::MpcConnection, start_timer};
use crypto_primitives::{
    cot::{client::num_additional_ot_needed, server::sample_chi},
    uint::UInt,
    utils::{iter_arc, log_verify_status, Hook},
};
use rayon::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::warn;

mod client_msg;
mod mpc;
mod utils;

type A = u64;

const CHI_SEED: u64 = 123456;

async fn main_with_options<I: UInt>(options: Options) {
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();

    // connect to peer
    let peer = if !cfg!(feature = "no-comm") {
        if options.is_bob {
            // I'm Bob and need a complete address of alice.
            MpcConnection::new_as_bob(&options.mpc_addr, options.num_mpc_sockets).await
        } else {
            // I'm Alice and I need a port number of alice.
            let mpc_addr =
                u16::from_str_radix(&options.mpc_addr, 10).expect("invalid mpc_addr as port");
            MpcConnection::new_as_alice(mpc_addr, options.num_mpc_sockets).await
        }
    } else {
        warn!("no-comm feature is enabled, so no communication with peers");
        MpcConnection::dummy()
    };

    let client_data =
        ClientData::<I>::fetch(options.is_alice(), options.client_port, options.num_clients).await;

    // manage message ids
    // for now, denote `a` as Alice (OT Sender) and `b` as Bob (OT Receiver)
    let ids = IdPool::build(
        client_data.num_clients_as_alice(),
        client_data.num_clients_as_bob(),
    );

    let timer = start_timer!(|| "OT Verify + B2A");

    // first, sample chi that is used to generate all OTs
    let num_ot = options.gsize * I::NUM_BITS as usize;
    let num_additional_ot = num_additional_ot_needed(num_ot);
    let chi = Arc::new(sample_chi(num_ot + num_additional_ot, CHI_SEED));

    // OT Verify Alice Receive (Start)
    let ot_alice_hook = Hook::new();
    let ot_ba_handles = iter_arc(&client_data.po2_msgs_alice)
        .zip(ids.otverify_a)
        .map(|(c_msg, id)| {
            let peer = peer.clone();
            let chi = chi.clone();
            tokio::spawn(async move { mpc::ot_verify_alice::<I>(id, &c_msg.cot, chi, peer).await })
        })
        .collect::<Vec<_>>();

    // OT verify Bob send (Start)

    let ot_bob_hook = Hook::new();
    let otverify_bob_handles = {
        let peer = peer.clone();
        let chi = chi.clone();
        let c_msg = client_data.po2_msgs_bob.clone();
        tokio::task::spawn_blocking(move || {
            c_msg
                .par_iter()
                .zip(ids.otverify_b)
                .map(|(c_msg, id)| mpc::ot_verify_bob(id, c_msg, &peer, chi.clone(), options.gsize))
                .collect::<Vec<_>>()
        })
    };

    // B2A Bob Receive (Start)
    let b2a_bob_hook = Hook::new();
    let b2a_bob_handles = iter_arc(&client_data.po2_msgs_bob)
        .zip(ids.b2a_b)
        .map(|(c_msg, id)| {
            let peer = peer.clone();
            tokio::spawn(async move { mpc::b2a_bob::<_, A>(id, &*c_msg, peer).await })
        })
        .collect::<Vec<_>>();

    // OT Verify Alice Receive (Complete)
    let mut qs_per_client = Vec::with_capacity(client_data.num_clients_as_alice());
    let mut num_verified_success = 0;
    for alice_handle in ot_ba_handles {
        let (qs, v) = alice_handle.await.unwrap();
        qs_per_client.push(qs);
        num_verified_success += v as usize;
    }
    log_verify_status(
        num_verified_success,
        client_data.num_clients_as_alice(),
        "OT Verify Alice",
    );
    ot_alice_hook.done();

    // B2A Alice Send (Start)
    let b2a_alice_hook = Hook::new();
    let b2a_alice_handles = tokio::task::block_in_place(|| {
        client_data
            .po2_msgs_alice
            .par_iter()
            .zip(qs_per_client)
            .zip(ids.b2a_a)
            .map(|((c_msg, qs), id)| mpc::b2a_alice::<I, A>(id, options.gsize, c_msg, &qs, &peer))
            .collect::<Vec<_>>()
    });

    // B2A Bob Receive (Complete)
    let mut bob_arith_shares = Vec::with_capacity(client_data.num_clients_as_bob());
    for bob_handle in b2a_bob_handles {
        let bob_arith_share = bob_handle.await.unwrap();
        bob_arith_shares.push(bob_arith_share);
    }
    b2a_bob_hook.done();

    // B2A Alice Send (Complete)
    let mut alice_arith_shares = Vec::with_capacity(client_data.num_clients_as_alice());
    for (s, handle) in b2a_alice_handles {
        handle.await.unwrap();
        alice_arith_shares.push(s);
    }
    b2a_alice_hook.done();

    // OT Verify Bob Send (Complete)
    for handle in otverify_bob_handles
        .await
        .expect("OT Verify on Bob part failed")
    {
        handle.await.unwrap();
    }
    ot_bob_hook.done();

    let b2a_time = end_timer!(timer).elapsed().as_secs_f64();

    println!("client comm, MPC comm, client phase 1, client phase 2, OT + B2A, Correlation verify, A2S, Hash verify");
    println!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        client_data.comm_alice + client_data.comm_bob,
        peer.num_bytes_received(),
        client_data.time,
        0f64,
        b2a_time,
        0f64,
        0f64,
        0f64
    );
}

pub fn main() {
    let options = Options::load_from_args("ELSA Server Po2");
    let runtime = Runtime::new().unwrap();
    match options.input_size {
        InputSize::U8 => {
            runtime.block_on(main_with_options::<u8>(options));
        },
        InputSize::U32 => runtime.block_on(main_with_options::<u32>(options)),
    }
}
