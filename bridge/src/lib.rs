#![deny(trivial_numeric_casts)]

use std::{
    fmt::{Debug, Display},
};

use thiserror::Error;
use tokio::net::{TcpStream, ToSocketAddrs};
use tracing::warn;
pub mod client_server;
pub mod id_tracker;
pub mod mpc_conn;
pub mod perf_trace;
/// Trait for abstract asynchronous connection
pub mod tcp_bridge;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("io Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    SerializationError(#[from] serialize::Error),
}

pub(crate) async fn tcp_connect_or_retry(
    remote_addr: impl ToSocketAddrs + Copy + Debug,
) -> TcpStream {
    let socket;
    loop {
        match TcpStream::connect(remote_addr).await {
            Ok(s) => {
                socket = s;
                break;
            },
            Err(e) => {
                warn!(
                    "Error connect to {:?}: {}. Waiting to connect in 100ms",
                    remote_addr, e
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            },
        }
    }
    socket
}

#[derive(Debug)]
/// Error due to server end.
pub struct ServerError(String);

impl Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("ServerError({})", self.0))
    }
}

impl std::error::Error for ServerError {}

// some useful auto-trait for simulation
pub trait BlackBox: Sized {
    fn black_box(self) -> Self {
        unsafe {
            let s = std::ptr::read_volatile(&self);
            std::mem::forget(self);
            s
        }
    }
    /// drop `self` without allowing compiler to optimize it away
    fn drop_into_black_box(self) {
        let _ = self.black_box();
    }
}

impl<T: Sized> BlackBox for T {}