use crate::{
    client_msg::ClientData,
    utils::{log_verify_status, HashPool, IdPool},
};
use bin_utils::server::{InputSize, Options};
use bridge::{
    client_server::ClientsPool, end_timer, mpc_conn::MpcConnection, start_timer, BlackBox,
};
use crypto_primitives::{
    cot::{client::num_additional_ot_needed, server::sample_chi},
    malpriv::MessageHash,
    uint::UInt,
    utils::{batch_xor, iter_arc, Hook},
    ALICE, BOB,
};
use rayon::prelude::*;
use sha2::Sha256;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::warn;

mod client_msg;
mod mpc;
mod utils;

type A = u64;
type C = u128;
type Hasher = Sha256;
fn make_hasher() -> Hasher {
    Hasher::default()
}

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

    let client_data = ClientData::<I, C, Hasher>::fetch(
        options.is_alice(),
        options.client_port,
        options.num_clients,
        options.gsize,
        make_hasher,
    )
    .await;

    // manage message ids
    // for now, denote `a` as Alice (OT Sender) and `b` as Bob (OT Receiver)
    let ids = IdPool::build(
        client_data.num_clients_as_alice(),
        client_data.num_clients_as_bob(),
    );

    // manage hashes
    let mut hashers = HashPool::init(
        client_data.num_clients_as_alice(),
        client_data.num_clients_as_bob(),
        make_hasher,
    );

    let timer = start_timer!(|| "Exchange seeds");
    let chi_seed_peer = peer
        .exchange_message(ids.exchange_chi_seed, &client_data.chi_seed_share)
        .await
        .unwrap();
    let t_seed_peer = peer
        .exchange_message(ids.exchange_t_seed, &client_data.t_seed_share)
        .await
        .unwrap();

    let chi_seed = batch_xor(&client_data.chi_seed_share, &chi_seed_peer);
    let t_seed = batch_xor(&client_data.t_seed_share, &t_seed_peer);
    let (t_seeds_a, t_seeds_b) = ClientsPool::split_iter(options.is_alice(), t_seed.into_iter());
    end_timer!(timer);

    let timer = start_timer!(|| "OT Verify + B2A");

    // first, sample chi that is used to generate all OTs
    let num_ot = options.gsize * I::NUM_BITS as usize;
    let num_additional_ot = num_additional_ot_needed(num_ot);
    let chis = chi_seed
        .par_iter()
        .map(|seed| sample_chi(num_ot + num_additional_ot, *seed))
        .collect::<Vec<_>>();
    let (chis_a, chis_b) = ClientsPool::split_iter(options.is_alice(), chis.into_iter());

    // OT Verify Alice Receive (Start)
    let ot_alice_hook = Hook::new();
    let ot_ba_handles = iter_arc(&client_data.po2_msgs_alice)
        .zip(ids.otverify_a)
        .zip(chis_a)
        .zip(hashers.ot_ba)
        .map(|(((c_msg, id), chi), mut hasher)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                let result =
                    mpc::ot_verify_alice::<I, _>(id, &c_msg.cot, Arc::new(chi), peer, &mut hasher)
                        .await;
                (result, hasher)
            })
        })
        .collect::<Vec<_>>();

    // OT verify Bob send (Start)

    let ot_bob_hook = Hook::new();
    let otverify_bob_handles = {
        let peer = peer.clone();
        let c_msg = client_data.po2_msgs_bob.clone();
        tokio::task::spawn_blocking(move || {
            c_msg
                .par_iter()
                .zip(ids.otverify_b)
                .zip(chis_b)
                .map(|((c_msg, id), chi)| {
                    mpc::ot_verify_bob(id, c_msg, &peer, Arc::new(chi), options.gsize)
                })
                .collect::<Vec<_>>()
        })
    };

    // B2A Bob Receive (Start)
    let b2a_bob_hook = Hook::new();
    let b2a_bob_handles = iter_arc(&client_data.po2_msgs_bob)
        .zip(ids.b2a_b)
        .zip(hashers.b2a_ab)
        .map(|((c_msg, id), mut hasher)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                let result = mpc::b2a_bob::<_, A, _>(id, &*c_msg, peer, &mut hasher).await;
                (result, hasher)
            })
        })
        .collect::<Vec<_>>();

    // OT Verify Alice Receive (Complete)
    let mut qs_per_client = Vec::with_capacity(client_data.num_clients_as_alice());
    let mut num_verified_success = 0;
    hashers.ot_ba = Vec::with_capacity(client_data.num_clients_as_alice());
    for alice_handle in ot_ba_handles {
        let ((qs, v), hasher) = alice_handle.await.unwrap();
        qs_per_client.push(qs);
        num_verified_success += v as usize;
        hashers.ot_ba.push(hasher);
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
                mpc::b2a_alice::<I, A>(id, options.gsize, c_msg.inputs_0, &c_msg.cot, &qs, &peer)
            })
            .collect::<Vec<_>>()
    });

    // B2A Bob Receive (Complete)
    let mut bob_arith_shares = Vec::with_capacity(client_data.num_clients_as_bob());
    hashers.b2a_ab = Vec::with_capacity(client_data.num_clients_as_bob());
    for bob_handle in b2a_bob_handles {
        let (bob_arith_share, hasher) = bob_handle.await.unwrap();
        bob_arith_shares.push(bob_arith_share);
        hashers.b2a_ab.push(hasher);
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

    let timer = start_timer!(|| "SqCorr Verify");
    assert!(client_data
        .sqcorr_alice
        .iter()
        .all(|corrs| corrs.len() == options.gsize * 2));
    assert!(client_data
        .sqcorr_bob
        .iter()
        .all(|corrs| corrs.len() == options.gsize * 2));

    let (sqcorr_a, sqcorr_b) = ClientsPool::split_iter(options.is_alice(), ids.sqcorr.into_iter());
    // SqCorr Verify
    let sqcorr_alice_handles = iter_arc(&client_data.sqcorr_alice)
        .zip(sqcorr_a)
        .zip(t_seeds_a)
        .zip(hashers.sqcorr_ba)
        .map(|(((corr, id), t_seed), mut hasher)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                let result = mpc::corr_verify::<_, ALICE, Hasher>(
                    id.0,
                    id.1,
                    options.gsize,
                    &*corr,
                    t_seed,
                    peer,
                    &mut hasher,
                )
                .await;
                (result, hasher)
            })
        })
        .collect::<Vec<_>>();
    let sqcorr_bob_handles = iter_arc(&client_data.sqcorr_bob)
        .zip(sqcorr_b)
        .zip(t_seeds_b)
        .zip(hashers.sqcorr_ab)
        .map(|(((corr, id), t_seed), mut hasher)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                let result = mpc::corr_verify::<_, BOB, Hasher>(
                    id.0,
                    id.1,
                    options.gsize,
                    &*corr,
                    t_seed,
                    peer,
                    &mut hasher,
                )
                .await;
                (result, hasher)
            })
        })
        .collect::<Vec<_>>();

    let mut num_verified_success = 0;
    hashers.sqcorr_ba = Vec::with_capacity(client_data.num_clients_as_alice());
    hashers.sqcorr_ab = Vec::with_capacity(client_data.num_clients_as_bob());
    for sqcorr_handle in sqcorr_alice_handles {
        let (result, hasher) = sqcorr_handle.await.unwrap();
        num_verified_success += if result == options.gsize { 1 } else { 0 };
        hashers.sqcorr_ba.push(hasher);
    }
    for sqcorr_handle in sqcorr_bob_handles {
        let (result, hasher) = sqcorr_handle.await.unwrap();
        num_verified_success += if result == options.gsize { 1 } else { 0 };
        hashers.sqcorr_ab.push(hasher);
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
    let sqcorr = ClientsPool::merge_msg(
        options.is_alice(),
        iter_arc(&client_data.sqcorr_alice),
        iter_arc(&client_data.sqcorr_bob),
    );
    let a2s_handles = sqcorr
        .into_iter()
        .zip(arith_shares)
        .zip(ids.a2s)
        .zip(hashers.a2s)
        .map(|(((corr, xs), id), mut hasher)| {
            let peer = peer.clone();
            tokio::spawn(async move {
                let result = if !options.is_bob {
                    mpc::a2s::<A, C, _, { ALICE }>(id, &xs, &*corr, peer, &mut hasher).await
                } else {
                    mpc::a2s::<_, _, _, { BOB }>(id, &xs, &*corr, peer, &mut hasher).await
                };
                (result, hasher)
            })
        })
        .collect::<Vec<_>>();

    hashers.a2s = Vec::with_capacity(client_data.num_clients());
    for handle in a2s_handles {
        let (result, hasher) = handle.await.unwrap();
        hashers.a2s.push(hasher);
        result.drop_into_black_box()
    }

    let a2s_time = end_timer!(timer).elapsed().as_secs_f64();

    let timer = start_timer!(|| "Hash Verification");
    // B2A
    assert_eq!(client_data.hash_b2a_ab.len(), hashers.b2a_ab.len());
    let num_verified = client_data
        .hash_b2a_ab
        .iter()
        .zip(hashers.b2a_ab)
        .map(|(expected, hasher)| {
            let actual = hasher.digest();
            (expected == &actual) as usize
        })
        .sum::<usize>();
    log_verify_status(
        num_verified,
        client_data.num_clients_as_bob(),
        "B2A Hash AB",
    );
    // A2S
    let num_verified = client_data
        .hash_a2s
        .iter()
        .zip(hashers.a2s)
        .map(|(expected, hasher)| {
            let actual = hasher.digest();
            (expected == &actual) as usize
        })
        .sum::<usize>();
    log_verify_status(num_verified, client_data.num_clients(), "A2S Hash");
    // OT Verify
    let num_verified = client_data
        .hash_ot_ba
        .iter()
        .zip(hashers.ot_ba)
        .map(|(expected, hasher)| {
            let actual = hasher.digest();
            (expected == &actual) as usize
        })
        .sum::<usize>();
    log_verify_status(
        num_verified,
        client_data.num_clients_as_alice(),
        "OT Verify Hash",
    );
    // SqCorr Verify
    assert_eq!(client_data.hash_sqcorr_ba.len(), hashers.sqcorr_ba.len());
    assert_eq!(client_data.hash_sqcorr_ab.len(), hashers.sqcorr_ab.len());
    let num_sqcorr_verified = client_data
        .hash_sqcorr_ba
        .iter()
        .chain(client_data.hash_sqcorr_ab.iter())
        .zip(
            hashers
                .sqcorr_ba
                .into_iter()
                .chain(hashers.sqcorr_ab.into_iter()),
        )
        .map(|(expected, hasher)| {
            let actual = hasher.digest();
            (expected == &actual) as usize
        })
        .sum::<usize>();

    log_verify_status(
        num_sqcorr_verified,
        client_data.num_clients(),
        "SqCorr Verify Hash",
    );
    let hash_verify_time = end_timer!(timer).elapsed().as_secs_f64();

    println!("client comm, MPC comm, client phase 1, client phase 2, OT + B2A, Correlation verify, A2S, Hash verify");
    println!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        client_data.comm_alice + client_data.comm_bob,
        peer.num_bytes_received(),
        client_data.phase1_time,
        client_data.phase2_time,
        b2a_time,
        corr_verify_time,
        a2s_time,
        hash_verify_time
    );
}

pub fn main() {
    let runtime = Runtime::new().unwrap();
    let options = Options::load_from_args("ELSA MP Server");
    match options.input_size {
        InputSize::U8 => runtime.block_on(main_with_option::<u8>(options)),
        InputSize::U32 => runtime.block_on(main_with_option::<u32>(options)),
    }
}
