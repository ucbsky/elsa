use crate::protocol::Client;
use bin_utils::{client::Options, InputSize};
use bridge::{
    client_server::init_meta_clients,
    end_timer,
    id_tracker::{RecvId, SendId},
    start_timer,
    tcp_bridge::TcpConnection,
};

use crypto_primitives::{malpriv::MessageHash, uint::UInt};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use tracing::info;

mod protocol;

type ARITH = u64;

fn hasher() -> impl MessageHash {
    sha2::Sha256::default()
}

pub async fn start_mp_client<I: UInt>(options: Options) {
    assert_eq!(options.input_size.num_bits(), I::NUM_BITS);
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();
    info!(
        "MP PO2 Client: num_clients: {}, Server address alice: {}, server address bob: {}, gsize: {}, tracing_level: {}",
        options.num_clients, options.server_alice, options.server_bob, options.gsize, options.log_level
    );

    let timer = start_timer!(|| "Preparing Client Input");
    let data = {
        (0..options.num_clients)
            .into_par_iter()
            .map(|i| {
                let mut rng = StdRng::seed_from_u64(i as u64);
                (0..options.gsize)
                    .map(|_| I::rand(&mut rng))
                    .collect::<Vec<I>>()
            })
            .collect::<Vec<Vec<I>>>()
    };
    end_timer!(timer);

    let mut rng = StdRng::from_entropy();
    let seeds = (0..options.num_clients)
        .map(|_| rng.gen::<u64>())
        .collect::<Vec<_>>();
    let timer = start_timer!(|| "Preparing Client Message");
    let clients = data
        .into_par_iter()
        .zip(seeds)
        .map(|(input, seed)| {
            Client::prepare_phase1::<ARITH, _, _>(&input, &mut StdRng::seed_from_u64(seed), hasher)
        })
        .collect::<Vec<Client<I, _>>>();
    end_timer!(timer);

    info!("Attempting to connect to server");
    let connections = init_meta_clients(
        options.num_clients,
        &options.server_alice,
        &options.server_bob,
    )
    .await;

    info!("All clients connected! Sending clients data...");

    // load balancing
    let arrange_conn = |a: TcpConnection, b: TcpConnection, uid: usize| {
        // alice is OT sender if uid is even
        let (alice, bob) = if uid % 2 == 0 { (a, b) } else { (b, a) };
        assert_eq!(alice.uid(), bob.uid());
        assert_eq!(alice.uid(), (uid as u64).into());
        (alice, bob)
    };

    let phase1_handles = clients
        .par_iter()
        .zip(connections.clone())
        .enumerate()
        .map(|(uid, (client, (server0, server1)))| {
            let (alice, bob) = arrange_conn(server0, server1, uid);
            let phase1_alice = client.send_to_alice(SendId::FIRST, alice);
            let phase1_bob = client.send_to_bob(SendId::FIRST, bob);
            [phase1_alice, phase1_bob]
        })
        .flatten()
        .collect::<Vec<_>>();

    let phase2_handles = clients
        .into_iter()
        .zip(connections)
        .enumerate()
        .map(|(uid, (client, (server0, server1)))| {
            let (alice, _) = arrange_conn(server0, server1, uid);
            tokio::spawn(async move {
                client
                    .phase_2::<ARITH, _>(alice, (RecvId::FIRST, SendId::SECOND), hasher)
                    .await
            })
        })
        .collect::<Vec<_>>();

    for h in phase1_handles {
        h.await.unwrap();
    }

    for h in phase2_handles {
        h.await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    let options = Options::load_from_args("ELSA Client (MP-Po2)");
    match options.input_size {
        InputSize::U8 => start_mp_client::<u8>(options).await,
        InputSize::U32 => start_mp_client::<u32>(options).await,
    }
}
