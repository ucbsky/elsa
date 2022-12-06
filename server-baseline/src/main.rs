use crate::protocol::prio_ring_sim_server;
use bin_utils::{server::Options, InputSize};
use bindings::ROTMode;
use bridge::{
    client_server::ClientsPool, end_timer, id_tracker::IdGen, mpc_conn::MpcConnection, start_timer,
    BlackBox,
};
use clap::Arg;
use crypto_primitives::{
    bits::{BitsLE, SeededInputShare},
    uint::UInt,
};
use rand::{rngs::StdRng, SeedableRng};
use rayon::prelude::*;
use serialize::UseCast;
use tokio::net::TcpListener;
use tracing::info;

mod protocol;
struct CustomOptions {
    mode: ROTMode,
    rot_port: i32,
}

async fn main_with_options<I: UInt>(options: Options<CustomOptions>) {
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();
    let listener = TcpListener::bind(("0.0.0.0", options.client_port))
        .await
        .unwrap();
    // accepts clients connection
    let clients = ClientsPool::new(options.num_clients, listener).await;

    let (clients_alice, clients_bob) = clients.split(options.is_alice());

    // connect to peer
    let peer = if !options.is_alice() {
        // I'm Bob and need a complete address of alice.
        MpcConnection::new_as_bob(&options.mpc_addr, options.num_mpc_sockets).await
    } else {
        // I'm Alice and I need a port number of alice.
        let mpc_addr =
            u16::from_str_radix(&options.mpc_addr, 10).expect("invalid mpc_addr as port");
        MpcConnection::new_as_alice(mpc_addr, options.num_mpc_sockets).await
    };

    let timer = start_timer!(|| "C->S");

    let alice_shares = clients_alice
        .subscribe_and_get::<UseCast<SeededInputShare>>(IdGen::new().next_recv_id())
        .await
        .unwrap();
    let alice_shares = alice_shares
        .into_par_iter()
        .map(|x| x.expand::<I>(options.gsize))
        .collect::<Vec<_>>();
    alice_shares.drop_into_black_box();
    let bob_shares = clients_bob
        .subscribe_and_get::<Vec<BitsLE<I>>>(IdGen::new().next_recv_id())
        .await
        .unwrap();
    bob_shares.drop_into_black_box();

    let client_time = end_timer!(timer).elapsed().as_secs_f64();

    info!(
        "Number of bytes received from clients: {}",
        clients.num_bytes_received_from_all()
    );

    let mut rng = StdRng::from_entropy();
    let timer = start_timer!(|| "MPC");
    let mpc_comm = prio_ring_sim_server::<I, u64, _>(
        &mut rng,
        clients.num_of_clients(),
        peer,
        (options.custom_args.rot_port
            ..options.custom_args.rot_port + options.num_mpc_sockets as i32)
            .step_by(2)
            .collect(),
        options.gsize,
        options.custom_args.mode,
    )
    .await;
    let mpc_time = end_timer!(timer).elapsed().as_secs_f64();

    info!("Number of bytes sent to peer: {}", mpc_comm);

    println!("client comm, MPC comm, client time, skip ,mpc, skip, skip, skip");
    println!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        clients.num_bytes_received_from_all(),
        mpc_comm,
        client_time,
        0f64,
        mpc_time,
        0f64,
        0f64,
        0f64
    );
}

#[tokio::main]
pub async fn main() {
    let options = Options::load_from_args_custom(
        "Baseline Simulation Server Using Prio+",
        [
            Arg::new("ferret")
                .short('f')
                .long("ferret")
                .help("set if we use Ferret ROT. otherwise use IKNP"),
            Arg::new("rot_port")
                .short('r')
                .long("rot_port")
                .help("port used for ROT")
                .takes_value(true)
                .default_value("8999"),
        ],
        |matches| {
            let mode = if matches.is_present("ferret") {
                ROTMode::FERRET
            } else {
                ROTMode::IKNP
            };
            let rot_port = i32::from_str_radix(matches.value_of("rot_port").unwrap(), 10).unwrap();

            CustomOptions { mode, rot_port }
        },
    );
    match options.input_size {
        InputSize::U8 => main_with_options::<u8>(options).await,
        InputSize::U32 => main_with_options::<u32>(options).await,
    }
}
