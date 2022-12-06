use crate::protocol::{start_one_round_client, Po2Client};
use bin_utils::{client::Options, InputSize};

mod protocol;

#[tokio::main]
pub async fn main() {
    let options = Options::load_from_args("ELSA Client (Po2)");
    match options.input_size {
        InputSize::U8 => start_one_round_client::<u8, Po2Client<_>>(options).await,
        InputSize::U32 => start_one_round_client::<u32, Po2Client<_>>(options).await,
    }
}
