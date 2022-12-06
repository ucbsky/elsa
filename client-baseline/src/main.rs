use crate::protocol::Client;
use bin_utils::{client::Options, InputSize};
use bridge::{client_server::init_meta_clients, end_timer, id_tracker::IdGen, start_timer};
use crypto_primitives::uint::UInt;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use tracing::info;

mod protocol;

async fn main_with_options<I: UInt>(options: Options) {
    assert_eq!(options.input_size.num_bits(), I::NUM_BITS);
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();

    info!(
        "Baseline Client: num_clients: {}, Server address alice: {}, server address bob: {}, gsize: {}, tracing_level: {}",
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
        .map(|(input, seed)| Client::new(&input, &mut StdRng::seed_from_u64(seed)))
        .collect::<Vec<_>>();
    end_timer!(timer);

    info!("Attempting to connect to server");
    let connections = init_meta_clients(
        options.num_clients,
        &options.server_alice,
        &options.server_bob,
    )
    .await;

    info!("All clients connected! Sending clients data...");

    let handles = clients
        .into_par_iter()
        .zip(connections)
        .enumerate()
        .map(|(_, (client, (conn_alice, conn_bob)))| {
            let h0 = client.send_to_server_0(IdGen::new().next_send_id(), conn_alice);
            let h1 = client.send_to_server_1(IdGen::new().next_send_id(), conn_bob);
            [h0, h1]
        })
        .flatten()
        .collect::<Vec<_>>();

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::main]
pub async fn main() {
    let options = Options::load_from_args("Baseline Simulation Client using Prio+");
    match &options.input_size {
        InputSize::U8 => main_with_options::<u8>(options).await,
        InputSize::U32 => main_with_options::<u32>(options).await,
    }
}
