mod data_prep;

use bin_utils::{client::Options, InputSize};
use bridge::{client_server::init_meta_clients, end_timer, id_tracker::SendId, start_timer};
use bytes::Bytes;
use crypto_primitives::uint::UInt;
use prio::field::Field64;
use rand::prelude::*;
use rayon::prelude::*;
use tracing::info;

type F = Field64;

fn prepare_data_message_naive<I: UInt>(options: &Options) -> Vec<(Bytes, Bytes)> {
    let mut rng = StdRng::from_entropy();

    let data = data_prep::prepare_data::<I, _>(options.gsize, &mut rng);
    let message = data_prep::prepare_message::<I, F>(&data);

    (0..options.num_clients).map(|_| message.clone()).collect()
}

async fn main_with_options<I: UInt>(options: Options) {
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();
    info!(
        "Number of clients: {}, Server address alice: {}, server address bob: {} , log_level: {}",
        options.num_clients, options.server_alice, options.server_bob, options.log_level
    );
    let timer = start_timer!(|| "Preparing data and message");
    let messages = prepare_data_message_naive::<I>(&options);
    end_timer!(timer);
    info!("Attempting to connect to server");
    let connections = init_meta_clients(
        options.num_clients,
        &options.server_alice,
        &options.server_bob,
    )
    .await;

    info!("All clients connected!");

    info!("Starting all client instances");

    let handles = connections
        .into_par_iter()
        .enumerate()
        .zip(messages)
        .map(|((idx, (server0, server1)), (msg_s0, msg_s1))| {
            // load balancing
            // even indices treat global alice as alice
            let (alice, bob) = if idx % 2 == 0 {
                (server0, server1)
            } else {
                (server1, server0)
            };
            let handle1 = alice.send_message_bytes(SendId::FIRST, msg_s0);
            let handle2 = bob.send_message_bytes(SendId::FIRST, msg_s1);
            (handle1, handle2)
        })
        .collect::<Vec<_>>();

    for (handle1, handle2) in handles {
        handle1.await.unwrap();
        handle2.await.unwrap();
    }
}

#[tokio::main]
async fn main() {
    let options = Options::load_from_args("Prio Baseline MP Client");
    match options.input_size {
        InputSize::U8 => main_with_options::<u8>(options).await,
        InputSize::U32 => main_with_options::<u32>(options).await,
    };
}
