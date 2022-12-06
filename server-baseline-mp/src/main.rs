use crate::server::basic_server;
use bin_utils::{server::Options, InputSize};
use bridge::{client_server::ClientsPool, mpc_conn::MpcConnection};
use clap::Arg;
use crypto_primitives::uint::UInt;
use prio::field::Field64;
use tokio::net::TcpListener;

mod server;

type F = Field64;
fn eval_at() -> F {
    F::from(12123)
}

struct CustomOptions {
    pub batch_size: usize,
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

    let (_, _, stat) = basic_server::<I, F>(
        options.is_bob,
        &clients,
        options.gsize,
        options.custom_args.batch_size,
        peer,
        eval_at(),
    )
    .await;
    let client_comm = clients.num_bytes_received_from_all();
    println!(
        "client comm, MPC comm, client time, skip ,mpc message prepare, mpc verify, skip, skip"
    );
    println!(
        "{}, {}, {}, {}, {}, {}, {}, {}",
        client_comm,
        stat.mpc_comm,
        stat.client_time,
        0f64,
        stat.mpc_prepare,
        stat.mpc_verify,
        0f64,
        0f64
    );
}

#[tokio::main]
async fn main() {
    let options = Options::load_from_args_custom(
        "server-baseline-mp",
        [Arg::new("batch")
            .long("batch")
            .takes_value(true)
            .help("batch size")
            .default_value("1024")],
        |m| {
            let batch_size = m
                .value_of("batch")
                .unwrap()
                .parse::<usize>()
                .expect("invalid batch size");

            CustomOptions { batch_size }
        },
    );
    match options.input_size {
        InputSize::U8 => main_with_options::<u8>(options).await,
        InputSize::U32 => main_with_options::<u32>(options).await,
    }
}
