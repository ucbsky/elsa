pub mod gf;

use std::ops::{BitAnd, BitXor, Not};

use bytemuck::{Pod, Zeroable};
use bytemuck_derive::TransparentWrapper;
use core::fmt::Debug;
use derive_more::{Binary, Display, LowerExp, LowerHex, UpperExp, UpperHex};
use rand::Rng;
use safe_arch::*;

use crate::Blocks;

/// An 128-bit block.
/// Internally represented as an 128-bit XMM vector. Computation is vectorized
/// using SSE2 and PCLMULQDQ intrinsics.
///
/// When represented as an element in GF128, the leftmost bit is the coefficient
/// of x^127, and the rightmost bit is the coefficient of x^0.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    TransparentWrapper,
    Display,
    Binary,
    LowerHex,
    UpperHex,
    LowerExp,
    UpperExp,
)]
pub struct Block(pub m128i);

impl Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Block({:x?})", self.0)
    }
}

unsafe impl Zeroable for Block {}
unsafe impl Pod for Block {}

impl BitAnd for Block {
    type Output = Block;

    fn bitand(self, rhs: Block) -> Block {
        Block(self.0 & rhs.0)
    }
}

impl BitXor for Block {
    type Output = Block;

    fn bitxor(self, rhs: Block) -> Block {
        Block(self.0 ^ rhs.0)
    }
}

impl Not for Block {
    type Output = Block;

    fn not(self) -> Self::Output {
        Block(!self.0)
    }
}

impl Block {
    /// Return a new block with bits uniformly distributed.
    pub fn rand<R: Rng>(rng: &mut R) -> Self {
        let val = rng.gen::<u128>();
        Self(val.into())
    }

    /// view the list of blocks as a slice of blocks. This operation is O(1)
    pub fn batch_cast_from_u8_slice(slice: &[u8]) -> &[Self] {
        bytemuck::cast_slice(slice)
    }

    /// view the list of blocks as a slice of blocks. This operation is O(1)
    pub fn batch_cast_from_u8_slice_mut(slice: &mut [u8]) -> &mut [Self] {
        bytemuck::cast_slice_mut(slice)
    }
}

impl Blocks for [Block] {
    fn as_u8_slice(&self) -> &[u8] {
        bytemuck::cast_slice(self)
    }

    fn as_u8_slice_mut(&mut self) -> &mut [u8] {
        bytemuck::cast_slice_mut(self)
    }
}

impl From<m128i> for Block{
    fn from(val: m128i) -> Self {
        Self(val)
    }
}

#[cfg(test)]
mod tests {
    use rand::{prelude::StdRng, Rng, SeedableRng};
    use safe_arch::m128i;

    use crate::{Block, Blocks};

    #[test]
    /// make sure CLMUL is supported

    fn test_valid_instruction() {
        let a = Block(safe_arch::set_i32_m128i(0b0, 0b0, 0b0, 0b101110));
        let b = Block(safe_arch::set_i32_m128i(0b0, 0b0, 0b0, 0b110101));
        let c = safe_arch::mul_i64_carryless_m128i::<0>(a.0, b.0);
        assert_eq!(safe_arch::extract_i32_imm_m128i::<0>(c), 0b11110110110);
        let d = Block(safe_arch::set_i32_m128i(0b0, 0b0, 0b0, 0b101110));
        assert_eq!(d, a);
        assert_ne!(d, b);

        println!("{:b}", c);
        let x = vec![0x12u8; 16];
        let k = safe_arch::load_unaligned_m128i(&bytemuck::cast_slice(x.as_slice())[0]);
        println!("{:b}", k);

        let mut rng = StdRng::seed_from_u64(12345);
        println!("{:b}", Block::rand(&mut rng));
    }

    #[test]
    /// This test makes sure that the randomness generated on x86 machine is
    /// consistent with that on other machines.
    fn test_rand_consistency() {
        let mut rng = StdRng::seed_from_u64(12345);
        let a = Block::rand(&mut rng);
        let mut rng = StdRng::seed_from_u64(12345);
        let b = rng.gen::<u128>();
        let b = Block(m128i::from(b));
        assert_eq!(a, b);
    }

    #[test]
    fn test_to_bytes() {
        let mut rng = StdRng::seed_from_u64(12345);
        let blocks = (0..37).map(|_| Block::rand(&mut rng)).collect::<Vec<_>>();

        let blocks_bytes = blocks.store_to_bytes();
        let blocks_from_bytes = Block::batch_cast_from_u8_slice(&blocks_bytes);

        assert_eq!(&blocks, blocks_from_bytes);
    }

    #[test]
    #[should_panic]
    fn unaligned_cast_should_fail() {
        let mut rng = StdRng::seed_from_u64(12345);
        let blocks = (0..37).map(|_| Block::rand(&mut rng)).collect::<Vec<_>>();

        let blocks_bytes = blocks.store_to_bytes();
        let _ = Block::batch_cast_from_u8_slice(&blocks_bytes[..blocks_bytes.len() - 1]);
    }
}
