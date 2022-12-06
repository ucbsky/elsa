//! Defined Block represented as GF(2^128) polynomial.

use std::io::{Read, Write};
use crate::Block;
use safe_arch::*;
use serialize::{AsUseCast, Communicate, UseCast};

impl Block {
    /// addition in GF(2^128)
    pub fn add_gf(self, other: Block) -> Block {
        self ^ other
    }

    /// multiplication of two blocks in GF(2^128) without modulo. Return an
    /// element in GF(2^256), represented as two blocks.
    /// Calculator: http://www.ee.unb.ca/cgi-bin/tervo/calc.pl?num=1100101&den=1101&f=m&e=1&m=1
    /// Adapted from: https://github.com/emp-toolkit/emp-tool/blob/d48e2b165e557d14a40e5918ef44dd646ae20bec/emp-tool/utils/f2k.h#L8-L24
    pub fn mul_gf_no_reduction(self, other: Block) -> GF2_256 {
        let mut tmp3 = mul_i64_carryless_m128i::<0x00>(self.0, other.0);
        let mut tmp4 = mul_i64_carryless_m128i::<0x10>(self.0, other.0);
        let mut tmp5 = mul_i64_carryless_m128i::<0x01>(self.0, other.0);
        let mut tmp6 = mul_i64_carryless_m128i::<0x11>(self.0, other.0);

        tmp4 = tmp4 ^ tmp5;
        tmp5 = byte_shl_imm_u128_m128i::<8>(tmp4);
        tmp4 = byte_shr_imm_u128_m128i::<8>(tmp4);
        tmp3 = tmp3 ^ tmp5;
        tmp6 = tmp6 ^ tmp4;

        GF2_256(Block(tmp3), Block(tmp6))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GF2_256(pub Block, pub Block);

impl GF2_256 {
    pub fn add_gf(self, other: GF2_256) -> GF2_256 {
        GF2_256(self.0.add_gf(other.0), self.1.add_gf(other.1))
    }
}

impl Communicate for GF2_256 {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        self.0.use_cast().size_in_bytes() + self.1.use_cast().size_in_bytes()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        self.0.use_cast().to_bytes(&mut dest);
        self.1.use_cast().to_bytes(&mut dest);
    }

    fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
        let a = UseCast::<Block>::from_bytes(&mut bytes)?;
        let b = UseCast::<Block>::from_bytes(&mut bytes)?;
        Ok(GF2_256(a, b))
    }
}

#[cfg(test)]
mod tests {
    use rand::{prelude::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_mul_gf_no_reduction() {
        let a = Block(0xdeadbeef12345678abcdef0123456789u128.into());
        let b = Block(0x1926371029371ab1928dfa02719a8c9du128.into());
        let GF2_256(r1_actual, r2_actual) = a.mul_gf_no_reduction(b);
        let (r1_expected, r2_expected) = (
            Block(0x85c715643121b006f26d0ee099b295f5u128.into()),
            Block(0x0bd81dd6e61ad2382b4bd5277202cd7cu128.into()),
        );
        assert_eq!(r1_actual, r1_expected);
        assert_eq!(r2_actual, r2_expected);
    }

    #[test]
    fn test_gf256_from_gf128() {}

    #[test]
    fn test_basic_law() {
        let mut rng = StdRng::seed_from_u64(12345);

        for _ in 0..1024 {
            let a = Block::rand(&mut rng);
            let b = Block::rand(&mut rng);
            let c = Block::rand(&mut rng);

            // anything * 0 = 0
            assert_eq!(
                a.mul_gf_no_reduction(Block(0u128.into())),
                GF2_256(Block(0u128.into()), Block(0u128.into()))
            );

            // a * 1 = a
            assert_eq!(
                a.mul_gf_no_reduction(Block(1u128.into())),
                GF2_256(a, Block(0u128.into()))
            );

            // a * b = b * a
            assert_eq!(a.mul_gf_no_reduction(b), b.mul_gf_no_reduction(a));

            // a * (b + c) = (a * b) + (a * c)
            let left = a.mul_gf_no_reduction(b.add_gf(c));
            let right_0 = a.mul_gf_no_reduction(b);
            let right_1 = a.mul_gf_no_reduction(c);
            let right = right_0.add_gf(right_1);
            assert_eq!(left, right);

            // (b + c) * a = b * a + c * a
            let left = b.add_gf(c).mul_gf_no_reduction(a);
            let right_0 = b.mul_gf_no_reduction(a);
            let right_1 = c.mul_gf_no_reduction(a);
            let right = right_0.add_gf(right_1);
            assert_eq!(left, right);
        }
    }
}
