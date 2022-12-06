//! crypto primitives for eiffel
#[deny(trivial_numeric_casts)]
#[macro_use]
pub mod utils;

pub mod a2s;
pub mod b2a;
pub mod bitmul;
pub mod bits;
pub mod block_crypto;
pub mod cot;
pub mod malpriv;
pub mod message;
pub mod square_corr;
pub mod uint;

// alice is server 0 (false), bob is server 1 (true)
pub const ALICE: bool = false;
pub const BOB: bool = true;
