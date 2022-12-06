//! Adapted from https://github.com/emp-toolkit/emp-tool/blob/master/emp-tool/utils/mitccrh.h

use crate::block_crypto::aes::{aes_opt_key_schedule, para_enc, AESKey};
use block::Block;
use safe_arch::{m128i, set_i64_m128i};

/// MiTCCR hash function
///
/// Reference: [GKWWY19](https://eprint.iacr.org/2019/1168)
#[derive(Clone, Debug)]
pub struct MiTCCR<const BATCH_SIZE: usize> {
    scheduled_key: [AESKey; BATCH_SIZE],
    keys: [m128i; BATCH_SIZE],
    // key_used: usize, // key_used is not used because each hash input length is same as key
    // length
    start_point: m128i,
    gid: u64,
}

impl<const BATCH_SIZE: usize> MiTCCR<BATCH_SIZE> {
    pub fn new(start_point: m128i) -> Self {
        MiTCCR {
            scheduled_key: [AESKey::default(); BATCH_SIZE],
            keys: [m128i::default(); BATCH_SIZE],
            start_point,
            gid: 0,
        }
    }

    /// renew keys
    pub fn renew_ks(&mut self) {
        let mut gid = self.gid;
        let start_point = self.start_point;
        self.keys.iter_mut().for_each(|k| {
            let tmp = set_i64_m128i(gid as i64, 0);
            gid += 1;
            *k = start_point ^ tmp;
        });
        self.gid = gid;

        aes_opt_key_schedule(&self.keys, &mut self.scheduled_key);
    }

    // correspond to https://github.com/emp-toolkit/emp-tool/blob/aaff3ab013861a8da3b11ade5fcab3a0655bf08d/emp-tool/utils/mitccrh.h#L45
    // but with K=BATCH_SIZE. So input length = K * H
    /// Calculate hash of input of certain length. The hash function applied on
    /// (`input[0]`, `input[1]`, ... , `input[H-1]`) are the same, and so on.
    ///
    /// # Panics (debug mode only)
    /// `input.len()` must be equal to `BATCH_SIZE * H`, otherwise panic.
    pub fn hash<const H: usize, const INPUT_SIZE: usize>(
        &mut self,
        input: &mut [m128i; INPUT_SIZE],
    ) {
        debug_assert_eq!(input.len(), INPUT_SIZE);
        debug_assert_eq!(input.len(), BATCH_SIZE * H);
        // we always renew_ks because we always use all keys in each hash
        self.renew_ks();

        let mut tmp = *input;
        para_enc::<H, BATCH_SIZE, INPUT_SIZE>(&mut tmp, &self.scheduled_key);

        input.iter_mut().zip(tmp.iter()).for_each(|(a, b)| {
            *a = *a ^ *b;
        });
    }

    /// Calculate hash of input of certain length. The hash function applied on
    /// (`input[0]`, `input[1]`, ... , `input[H-1]`) are the same, and so on.
    ///
    /// # Panics (debug mode only)
    /// `input.len()` must be equal to `BATCH_SIZE * H`, otherwise panic.
    pub fn hash_block<const H: usize, const INPUT_SIZE: usize>(
        &mut self,
        input: &mut [Block; INPUT_SIZE],
    ) {
        self.hash::<H, INPUT_SIZE>(bytemuck::cast_mut(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    #[test]
    fn sanity_test_one_hash() {
        const BATCH_SIZE: usize = 8;
        let mut rng = StdRng::seed_from_u64(0);
        let mut crh1 = MiTCCR::<BATCH_SIZE>::new(Block::rand(&mut rng).0);
        let mut crh2 = crh1.clone();
        let mut crh3 = crh1.clone();

        // design some input so that input1 = input2, input1 != input3
        let mut input1 = [Block::default(); BATCH_SIZE];
        input1.iter_mut().for_each(|x| *x = Block::rand(&mut rng));

        let mut input2 = input1;

        let mut input3 = [Block::default(); BATCH_SIZE];
        input3.iter_mut().for_each(|x| *x = Block::rand(&mut rng));

        crh1.hash_block::<1, BATCH_SIZE>(&mut input1);
        crh2.hash_block::<1, BATCH_SIZE>(&mut input2);
        crh3.hash_block::<1, BATCH_SIZE>(&mut input3);

        assert_ne!(input1, input3);
        assert_eq!(input1, input2);
    }

    #[test]
    // test when H=2
    fn sanity_test_two_hash() {
        const BATCH_SIZE: usize = 8;
        let mut rng = StdRng::seed_from_u64(0);
        let mut crh1 = MiTCCR::<BATCH_SIZE>::new(Block::rand(&mut rng).0);
        let mut crh2 = crh1.clone();
        let mut crh3 = crh1.clone();

        // design some input so that input1 = input2, input1 != input3
        let mut input1 = [Block::default(); BATCH_SIZE * 2];
        input1.iter_mut().for_each(|x| *x = Block::rand(&mut rng));

        let mut input2 = input1;

        let mut input3 = [Block::default(); BATCH_SIZE * 2];
        input3.iter_mut().for_each(|x| *x = Block::rand(&mut rng));

        crh1.hash_block::<2, { BATCH_SIZE * 2 }>(&mut input1);
        crh2.hash_block::<2, { BATCH_SIZE * 2 }>(&mut input2);
        crh3.hash_block::<2, { BATCH_SIZE * 2 }>(&mut input3);

        assert_ne!(input1, input3);
        assert_eq!(input1, input2);
    }

    #[test]
    fn test_h_hash_consistency() {
        const BATCH_SIZE: usize = 4;

        // we have an array
        // A=[0,1,2,3,4,5,6,7], where H=2, hash it and got [h0, h1, h2, h3, h4, h5, h6,
        // h7] we have a sub-array
        // B=[1,2,4,6], where H=1, hash it and should got [h1, h2, h4, h6]
        // we have a sub-array
        // C=[0,2,5,7], where H=1, hash it and should got [h0, h2, h5, h7]

        const H_A: usize = 2;
        const INPUT_SIZE_A: usize = BATCH_SIZE * H_A;

        const H_B: usize = 1;
        const INPUT_SIZE_B: usize = BATCH_SIZE * H_B;

        const H_C: usize = 1;
        const INPUT_SIZE_C: usize = BATCH_SIZE * H_C;

        let mut rng = StdRng::seed_from_u64(0);
        let start_point = Block::rand(&mut rng);
        let mut crh_a = MiTCCR::<BATCH_SIZE>::new(start_point.0);
        let mut crh_b = crh_a.clone();
        let mut crh_c = crh_a.clone();

        let mut a = [Block::default(); INPUT_SIZE_A];
        a.iter_mut().for_each(|x| *x = Block::rand(&mut rng));
        let mut b = [a[1], a[2], a[4], a[6]];
        let mut c = [a[0], a[2], a[5], a[7]];

        crh_a.hash_block::<H_A, INPUT_SIZE_A>(&mut a);
        crh_b.hash_block::<H_B, INPUT_SIZE_B>(&mut b);
        crh_c.hash_block::<H_C, INPUT_SIZE_C>(&mut c);

        let b_expected = [a[1], a[2], a[4], a[6]];
        let c_expected = [a[0], a[2], a[5], a[7]];

        assert_eq!(b_expected, b, "mismatch: b");
        assert_eq!(c_expected, c, "mismatch: c");
    }
}
