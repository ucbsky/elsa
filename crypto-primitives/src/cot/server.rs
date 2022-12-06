//! Server side code for ROT

use crate::{
    bits::{BitsLE},
    block_crypto::rng::BlockRng,
    cot::COTSeed,
    uint::UInt,
};
use block::{gf::GF2_256, Block};


use super::ChoiceSeed;

/// Freshly sample coefficients for OT Verification.
#[inline]
pub fn sample_chi(num_ots: usize, shared_seed: u64) -> Vec<Block> {
    let mut rng = BlockRng::new(Some(Block([shared_seed, 0].into())));
    let mut chi = vec![Block::default(); num_ots];
    rng.random_blocks(&mut chi);
    chi
}

pub struct OTReceiver {}

impl OTReceiver {
    /// Send x_til and t_til according to paper.
    /// ```pseudocode
    /// r = r_seed
    /// x_hat = (xs || r)
    /// x_til = x_hat.dot(chi)
    /// t_til = ts.dot(chi)
    /// ```
    /// * `chi`: random coefficients for OT
    /// * `xs`: boolean share for inputs
    /// * `ts`: received OT
    /// * `r_seed`: random seed for to generate r, for x_hat
    ///
    /// Returns `x_til` and `t_til`
    #[must_use]
    pub fn send_x_til_t_til<B: UInt>(
        ts: &[Block],
        chi: &[Block],
        inputs_1: &[BitsLE<B>],
        r_seed: ChoiceSeed,
    ) -> (Block, GF2_256) {
        // sanity check: chi and ts should have same length, length of xs should be <=
        // length of chi
        assert_eq!(chi.len(), ts.len());
        assert!(inputs_1.len() <= chi.len());

        // generate x_hat
        let r_size = chi.len() - inputs_1.len() * B::NUM_BITS;
        let r = r_seed.expand(r_size);
        let x_hat = inputs_1.iter().map(|x| x.iter()).flatten().chain(r.iter());

        let x_til = inner_product_with_boolean_scalar(x_hat, chi);

        let t_til = inner_product(ts, chi);

        (x_til, t_til)
    }
}

pub struct OTSender {}

impl OTSender {
    /// Verify if OT is correct, given OT receiver's message.
    /// ```pseudocode
    /// q_til = qs.dot(chi)
    /// lhs = t_til
    /// rhs = q_til + delta * x_til
    /// return lhs == rhs
    /// ```
    ///
    /// Return `qs`, which is first COT.
    ///
    /// # Panics
    /// This function panics if verification fails.
    pub fn verify_and_get_cot(
        qs_seed: COTSeed,
        chi: &[Block],
        delta: Block,
        x_til: Block,
        t_til: GF2_256,
    ) -> (Vec<Block>, bool) {
        let num_cots = chi.len();
        let qs = qs_seed.expand(num_cots);
        // sanity check: chi and qs should have same length
        let q_til = inner_product(&qs, chi);
        let lhs = t_til;
        let rhs = q_til.add_gf(delta.mul_gf_no_reduction(x_til));

        (qs, lhs == rhs)
    }
}

/// Calculate `a.dot(b)` where `a` is a vector of booleans in packed format, and
/// `b` is a slice of GF(2^128) blocks.
pub fn inner_product_with_boolean_scalar(a: impl Iterator<Item = bool>, b: &[Block]) -> Block {
    a.zip(b).fold(Block::default(), |prev, (left, right)| {
        if left {
            prev.add_gf(*right)
        } else {
            prev
        }
    })
}

pub fn inner_product(a: &[Block], b: &[Block]) -> GF2_256 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .fold(GF2_256::default(), |prev, (left, right)| {
            prev.add_gf(left.mul_gf_no_reduction(*right))
        })
}

#[cfg(test)]
mod tests {
    use crate::{
        cot::{
            client::{num_additional_ot_needed, COTGen},
            server::{sample_chi, OTReceiver, OTSender},
        },
        uint::UInt,
    };
    use rand::{rngs::StdRng, Rng, SeedableRng};

    #[test]
    fn verify_end_to_end() {
        let mut rng = StdRng::seed_from_u64(0);

        let inputs_1 = (0..1024)
            .map(|_| rng.gen::<u32>().bits_le())
            .collect::<Vec<_>>(); // known by clients and OT receiver
        let num_additional_ots = num_additional_ot_needed(inputs_1.len());

        // client samples COTs
        let delta = COTGen::sample_delta(&mut rng);
        let (msg_to_cx, msg_to_rx) =
            COTGen::sample_cots(&mut rng, &inputs_1, delta, num_additional_ots);

        // both server share the same chi
        let chi = sample_chi(
            inputs_1.len() * u32::NUM_BITS + num_additional_ots,
            1234567,
        );

        // OT receiver knows the choice bits (which is the same as its input boolean
        // share)
        let (x_til, t_til) =
            OTReceiver::send_x_til_t_til(&msg_to_rx.ts, &chi, &inputs_1, msg_to_rx.r_seed);

        // OT sender verifies the COT using OT receiver's message
        let (_, b) = OTSender::verify_and_get_cot(msg_to_cx.qs_seed, &chi, delta, x_til, t_til);
        assert!(b)

        // should not panic
    }
}
