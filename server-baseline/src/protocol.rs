use bindings::{get_rot_emp_dummy, ROTMode, RotConfig};
use bridge::{end_timer, id_tracker::IdGen, mpc_conn::MpcConnection, start_timer};
use crypto_primitives::uint::UInt;
use rand::{prelude::*, Rng};
use std::ffi::CString;
use tracing::info;

/// FL Server that uses Ferret ROT to generate beaver triples.
/// server id is 0 if b is false, otherwise it is 1.
pub async fn prio_ring_sim_server<I: UInt, A: UInt, R: Rng>(
    rng: &mut R,
    num_clients: usize,
    peer: MpcConnection,
    rot_ports: Vec<i32>,
    gsize: usize,
    rot_mode: ROTMode,
) -> usize {
    // track the message id with client, and message id with peer
    let mut peer_id_gen = IdGen::new();

    // Stage 2: Dummy OT Phase
    // TODO: check how many OTs should we need? Does it depend on input size?
    let timer = start_timer!(|| "Dummy OT Phase");
    let num_ots_needed_for_each_clients = I::NUM_BITS as usize * gsize;
    let total_ots_needed = num_clients * num_ots_needed_for_each_clients;
    let num_ots_for_each_port = total_ots_needed / rot_ports.len();
    let rot_handles = rot_ports
        .clone()
        .into_iter()
        .map(|port| {
            let handle1 = tokio::task::spawn_blocking(move || {
                get_rot_emp_dummy(
                    (num_ots_for_each_port / 2) as i64,
                    &RotConfig::Alice(port),
                    rot_mode,
                )
            });
            let peer_cloned = peer.clone();
            let handle2 = tokio::task::spawn_blocking(move || {
                let peer_addr = peer_cloned.ip_addr().to_string();
                get_rot_emp_dummy(
                    (num_ots_for_each_port / 2) as i64,
                    &RotConfig::Bob(CString::new(peer_addr.as_str()).unwrap(), port),
                    rot_mode,
                )
            });
            (handle1, handle2)
        })
        .collect::<Vec<_>>();

    let mut total_sent: u64 = 0;
    for (handle1, handle2) in rot_handles {
        let n1 = handle1.await.unwrap();
        let n2 = handle2.await.unwrap();
        total_sent += n1 + n2;
    }

    end_timer!(timer);

    // TODO: check how many data sent
    let timer = start_timer!(|| "Dummy Data sending phase");
    let data_need_to_sent = gsize * I::NUM_BITS * A::NUM_BITS / 8;
    info!("Data need to sent: {}", data_need_to_sent);

    let handles = (0..num_clients / 2)
        .map(|_| {
            let peer = peer.clone();
            let mut rng = StdRng::from_seed(rng.gen());
            let mut peer_id_gen = peer_id_gen.reserve_rounds(10);
            tokio::spawn(async move {
                //
                let dummy_bytes = (0..data_need_to_sent)
                    .map(|_| u8::rand(&mut rng))
                    .collect::<Vec<_>>();
                peer.exchange_message(peer_id_gen.next_exchange_id(), &dummy_bytes)
                    .await
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.await.unwrap();
    }

    end_timer!(timer);

    let total_bytes_sent_in_dummy = peer.num_bytes_sent();
    let total_bytes = total_bytes_sent_in_dummy + total_sent as usize;

    total_bytes
}
