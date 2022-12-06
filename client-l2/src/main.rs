use crate::protocol::L2Client;

use bin_utils::{client::Options, InputSize};
use client_po2::protocol::start_one_round_client;

mod protocol;

type CORR = u128;

#[tokio::main]
pub async fn main() {
    let options = Options::load_from_args("ELSA Client (L2)");
    match options.input_size {
        InputSize::U8 => start_one_round_client::<u8, L2Client<_, CORR>>(options).await,
        InputSize::U32 => start_one_round_client::<u32, L2Client<_, CORR>>(options).await,
    }
}
