pub use crate::InputSize;
use clap::{Arg, Command};
pub struct Options {
    pub server_alice: String,
    pub server_bob: String,
    pub num_clients: usize,
    pub gsize: usize,
    pub log_level: tracing_core::Level,
    pub input_size: InputSize,
}

impl Options {
    pub fn load_from_args(program_name: &str) -> Self {
        let matches = Command::new(program_name)
            .version("0.1")
            .arg(
                Arg::new("server_alice")
                    .short('a')
                    .long("server-alice")
                    .default_value("localhost:6666")
                    .takes_value(true)
                    .help("address of server slice (b=0)"),
            )
            .arg(
                Arg::new("server_bob")
                    .short('b')
                    .long("server-bob")
                    .default_value("localhost:6667")
                    .takes_value(true)
                    .help("address of server slice (b=1)"),
            )
            .arg(
                Arg::new("num_clients")
                    .short('n')
                    .long("num-clients")
                    .takes_value(true)
                    .required(true)
                    .help("number of clients to run"),
            )
            .arg(
                Arg::new("gsize")
                    .short('g')
                    .long("gsize")
                    .takes_value(true)
                    .required(true)
                    .help("number of inputs"),
            )
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("whether to show verbose output"),
            )
            .arg(
                Arg::new("input_size")
                    .short('i')
                    .long("input-size")
                    .takes_value(true)
                    .default_value("8")
                    .help("input size"),
            )
            .get_matches();

        let log_level = if matches.is_present("verbose") {
            tracing_core::Level::DEBUG
        } else {
            tracing_core::Level::INFO
        };

        let server_alice = matches.value_of("server_alice").unwrap();
        let server_bob = matches.value_of("server_bob").unwrap();

        let num_clients = matches
            .value_of("num_clients")
            .unwrap()
            .parse::<usize>()
            .unwrap();

        let gsize = matches.value_of("gsize").unwrap().parse::<usize>().unwrap();
        let input_size = matches
            .value_of("input_size")
            .unwrap()
            .parse::<InputSize>()
            .unwrap();

        Options {
            server_alice: server_alice.to_string(),
            server_bob: server_bob.to_string(),
            num_clients,
            gsize,
            log_level,
            input_size,
        }
    }
}
