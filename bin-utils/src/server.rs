pub use crate::InputSize;
use clap::{Arg, ArgMatches, Command};

pub struct Options<C = ()> {
    pub client_port: u16,
    pub num_clients: usize,
    pub gsize: usize,
    pub is_bob: bool,
    pub mpc_addr: String,
    pub num_mpc_sockets: usize,
    pub log_level: tracing_core::Level,
    pub input_size: InputSize,
    pub custom_args: C,
}

impl<C> Options<C> {
    pub fn load_from_args_custom<'a, P>(
        program_name: &str,
        custom_args: impl IntoIterator<Item = Arg<'a>>,
        parser: P,
    ) -> Self
    where
        P: FnOnce(&ArgMatches) -> C,
    {
        let mut builder = Command::new(program_name)
            .version("0.1")
            .arg(
                Arg::new("port")
                    .short('p')
                    .long("port")
                    .takes_value(true)
                    .help("port to listen to clients"),
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
                Arg::new("bob")
                    .short('b')
                    .long("bob")
                    .help("run as Bob if this flag is set, otherwise alice"),
            )
            .arg(
                Arg::new("mpc_addr")
                    .short('m')
                    .long("mpc_addr")
                    .takes_value(true)
                    .required(true)
                    .help("address of alice (should be a port number if I'm alice, otherwise, should be a complete address)")
            )
            .arg(
                Arg::new("num_mpc_sockets")
                    .short('s')
                    .long("num_mpc_sockets")
                    .takes_value(true)
                    .default_value("16")
                    .help("number of mpc sockets to use")
            )
            .arg(Arg::new("input_size")
                .short('i')
                .long("input_size")
                .takes_value(true)
                .default_value("8")
                .help("size of input"))
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("whether to show verbose output"),
            );
        for arg in custom_args {
            builder = builder.arg(arg);
        }
        let matches = builder.get_matches();

        let num_clients = matches
            .value_of("num_clients")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let gsize = matches.value_of("gsize").unwrap().parse::<usize>().unwrap();
        let is_bob = matches.is_present("bob");
        let client_port = matches
            .value_of("port")
            .unwrap_or_else(|| if is_bob { "6666" } else { "6667" })
            .parse::<u16>()
            .unwrap();
        let mpc_addr = matches.value_of("mpc_addr").unwrap().to_string();
        let num_mpc_sockets = matches
            .value_of("num_mpc_sockets")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let tracing_level = if matches.is_present("verbose") {
            tracing_core::Level::DEBUG
        } else {
            tracing_core::Level::INFO
        };
        let input_size = matches
            .value_of("input_size")
            .unwrap()
            .parse::<InputSize>()
            .unwrap();
        let custom_args = parser(&matches);

        Options {
            client_port,
            num_clients,
            gsize,
            is_bob,
            mpc_addr,
            num_mpc_sockets,
            log_level: tracing_level,
            input_size,
            custom_args,
        }
    }

    pub fn is_alice(&self) -> bool {
        !self.is_bob
    }
}

impl Options {
    /// Loads the command line options.
    pub fn load_from_args(program_name: &str) -> Self {
        Self::load_from_args_custom(program_name, [], |_| ())
    }
}
