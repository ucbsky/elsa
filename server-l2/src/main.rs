use crate::{client_msg::ClientData, utils::IdPool};
use bin_utils::server::{InputSize, Options};
use bridge::{
    client_server::ClientsPool, end_timer, mpc_conn::MpcConnection, start_timer, BlackBox,
};
use crypto_primitives::{
    cot::{client::num_additional_ot_needed, server::sample_chi},
    uint::UInt,
    utils::{iter_arc, log_verify_status, Hook},
    ALICE, BOB,
};
use rand::{rngs::StdRng, SeedableRng};
use rayon::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::warn;

mod client_msg;
mod mpc;
mod utils;

type A = u64;
type C = u128;

const CHI_SEED: u64 = 123456;

async fn main_with_option<I: UInt>(options: Options) {
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

    let client_data = ClientData::<I, C>::fetch(
        options.is_alice(),
        options.client_port,
        options.num_clients,
        options.gsize,
    )
    .await;

    // manage message ids
    // for now, denote `a` as Alice (OT Sender) and `b` as Bob (OT Receiver)
    let ids = IdPool::build(
        client_data.num_clients_as_alice(),
        client_data.num_clients_as_bob(),
    );

    let timer = start_timer!(|| "OT Verify + B2A");

    let (alice_arith_shares, bob_arith_shares) = if !cfg!(feature = "no-ot") {
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
                tokio::spawn(
                    async move { mpc::ot_verify_alice::<I>(id, &c_msg.cot, chi, peer).await },
                )
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
                    .map(|(c_msg, id)| {
                        mpc::ot_verify_bob(id, c_msg, &peer, chi.clone(), options.gsize)
                    })
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
                .map(|((c_msg, qs), id)| {
                    mpc::b2a_alice::<I, A>(id, options.gsize, c_msg, &qs, &peer)
                })
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

        (alice_arith_shares, bob_arith_shares)
    } else {
        let alice_arith_shares = (0..client_data.num_clients_as_alice())
            .into_par_iter()
            .map(|_| {
                let mut dummy_rng = StdRng::from_entropy();
                (0..options.gsize)
                    .map(|_| A::rand(&mut dummy_rng))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let bob_arith_shares = (0..client_data.num_clients_as_bob())
            .into_par_iter()
            .map(|_| {
                let mut dummy_rng = StdRng::from_entropy();
                (0..options.gsize)
                    .map(|_| A::rand(&mut dummy_rng))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        (alice_arith_shares, bob_arith_shares)
    };

    let b2a_time = end_timer!(timer).elapsed().as_secs_f64();

    let timer = start_timer!(|| "SqCorr Verify");
    // sanity checks: length check
    assert_eq!(client_data.sqcorr.len(), options.num_clients);
    assert!(client_data
        .sqcorr
        .iter()
        .all(|corrs| corrs.len() == options.gsize * 2));
    // SqCorr Verify
    let sqcorr_handles = iter_arc(&client_data.sqcorr)
        .zip(ids.sqcorr)
        .map(|(corr, id)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                if !options.is_bob {
                    mpc::corr_verify::<_, ALICE>(id.0, id.1, options.gsize, &*corr, peer).await
                } else {
                    mpc::corr_verify::<_, BOB>(id.0, id.1, options.gsize, &*corr, peer).await
                }
            })
        })
        .collect::<Vec<_>>();

    let mut num_verified_success = 0;
    for sqcorr_handle in sqcorr_handles {
        let result = sqcorr_handle.await.unwrap();
        num_verified_success += if result == options.gsize { 1 } else { 0 };
    }

    log_verify_status(
        num_verified_success,
        client_data.num_clients(),
        "SqCorr Verify",
    );

    let corr_verify_time = end_timer!(timer).elapsed().as_secs_f64();

    let timer = start_timer!(|| "A2S");
    // A2S
    let arith_shares = ClientsPool::merge_msg(
        options.is_alice(),
        alice_arith_shares.into_iter(),
        bob_arith_shares.into_iter(),
    );
    let a2s_handles = iter_arc(&client_data.sqcorr)
        .zip(arith_shares)
        .zip(ids.a2s)
        .map(|((corr, xs), id)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                if !options.is_bob {
                    mpc::a2s::<A, C, { ALICE }>(id, &xs, &*corr, peer).await
                } else {
                    mpc::a2s::<_, _, { BOB }>(id, &xs, &*corr, peer).await
                }
            })
        })
        .collect::<Vec<_>>();

    for handle in a2s_handles {
        handle.await.unwrap().drop_into_black_box()
    }

    let a2s_time = end_timer!(timer).elapsed().as_secs_f64();

    println!("client comm, MPC comm, client phase 1, client phase 2, OT + B2A, Correlation verify, A2S, Hash verify");
    println!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        client_data.comm_alice + client_data.comm_bob,
        peer.num_bytes_received(),
        client_data.time,
        0f64,
        if cfg!(feature = "no-ot") {
            0f64
        } else {
            b2a_time
        },
        corr_verify_time,
        a2s_time,
        0f64
    );
}

pub fn main() {
    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let options = Options::load_from_args("ELSA Server L2");
        match options.input_size {
            InputSize::U8 => {
                main_with_option::<u8>(options).await;
            },
            InputSize::U32 => {
                main_with_option::<u32>(options).await;
            },
        }
    })
}
