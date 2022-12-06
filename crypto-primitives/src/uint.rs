//! Defines Hash functions that takes input as block.

use crate::bits::BitsLE;
use bytemuck::{Pod, Zeroable};
use num_traits::{PrimInt, Unsigned, WrappingAdd, WrappingMul, WrappingNeg, WrappingSub};
use rand::Rng;
use safe_arch::{get_i32_from_m128i_s, get_i64_from_m128i_s, m128i};
use std::{
    any::Any,
    fmt::{Binary, Debug, Display, LowerHex, UpperHex},
    iter::Sum,
};

pub trait UInt:
    Unsigned
    + PrimInt
    + WrappingNeg
    + WrappingAdd
    + WrappingSub
    + WrappingMul
    + Sum
    + Send
    + Sync
    + Any
    + Debug
    + Pod
    + Zeroable
    + Display
    + Binary
    + LowerHex
    + UpperHex
{
    const NUM_BITS: usize;
    
    fn rand<R: Rng>(rng: &mut R) -> Self;
    /// Generate a random number at range range.0..range.1
    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self;
    /// From ROT Block.
    fn from_rot(block: m128i) -> Self;
    /// if true then 1 else 0
    fn from_bool(b: bool) -> Self;

    /// Convert `self` to little endian bits, at zero cost.
    fn bits_le(self) -> BitsLE<Self> {
        BitsLE(self)
    }

    /// `self % (2^bit_length)`
    #[must_use]
    fn modulo_2_power(self, bit_length: usize) -> Self;

    /// generate arithmetic shares of `self`
    fn arith_shares<R: Rng>(self, rng: &mut R) -> (Self, Self){
        let s0 = Self::rand(rng);
        let s1 = self.wrapping_sub(&s0);
        (s0, s1)
    }

    /// Encode this integer with interval bound shares.
    ///
    /// Returns:
    /// * `y` that has bit sizes as `wsize - 1`, which is
    ///   `biggest_interval_size`.
    /// * `s` that has bit sizes as `hsize`, which is the hamming weight of
    ///   bound and number of intervals. Assume `s_dest` is initialized to
    ///   false.
    ///
    ///  for simplicity, the internal representation of `y` and `s` uses
    /// `NUM_BITS` instead of `wsize - 1` and `hsize`. This should be considered
    /// as a minor overhead, because most communication is in COT.
    fn to_bounded_encoding(self, bound: Self) -> (BitsLE<Self>, BitsLE<Self>) {
        debug_assert!(!bound.is_zero());
        debug_assert!(self < bound);

        // suppose the bound is 0b1010101
        // the first interval is in range 0b0xxxxxx
        // the second interval is 0b100xxxx
        // the third interval is 0b10100xx
        // the fourth interval is 0b101010x
        //
        // we first find the first bit from MSB such that num is 0 and bound is 1. The
        // location of such bit is the interval size, i.e. number of `x` in the
        // interval
        let current_interval_size =
            Self::NUM_BITS - (((self ^ bound) & bound).leading_zeros() as usize) - 1;
        // we will need to keep the size of y the same no matter which interval our
        // input value lies in for security reasons.

        let y = BitsLE(self & ((Self::one() << current_interval_size as usize) - Self::one()));
        let which_interval = ((Self::one() << (current_interval_size as usize)) - Self::one())
            .not()
            .bitand(bound)
            .count_ones()
            - 1;
        let s = BitsLE(Self::one() << which_interval as usize);
        (y, s)
    }

    #[inline]
    fn wsize(self) -> usize {
        Self::NUM_BITS - self.leading_zeros() as usize
    }
    
    /// Cut `Self` to `T`. If `T` has fewer bits than `Self`, take the lower bits.
    #[inline]
    fn as_uint<T: UInt>(self) -> T{
        if T::NUM_BITS < Self::NUM_BITS{
            let t = self.modulo_2_power(T::NUM_BITS);
            T::from(t).unwrap()
        }else{
            T::from(self).unwrap()
        }
    }
}

impl UInt for u16 {
    const NUM_BITS: usize = u16::BITS as usize;


    fn rand<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }

    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self {
        rng.gen_range(range.0..range.1)
    }

    fn from_rot(block: m128i) -> Self {
        (get_i32_from_m128i_s(block) & 0xffff) as u16
    }

    fn from_bool(b: bool) -> Self {
        b as Self
    }

    fn modulo_2_power(self, bit_length: usize) -> Self {
        self & ((1 << bit_length) - 1)
    }


}

impl UInt for u32 {
    const NUM_BITS: usize = u32::BITS as usize;

    fn rand<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }

    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self {
        rng.gen_range(range.0..range.1)
    }

    fn from_rot(block: m128i) -> Self {
        get_i32_from_m128i_s(block) as u32
    }

    fn from_bool(b: bool) -> Self {
        b as Self
    }

    fn modulo_2_power(self, bit_length: usize) -> Self {
        self & ((1 << bit_length) - 1)
    }
}

impl UInt for u64 {
    const NUM_BITS: usize = u64::BITS as usize;

    fn rand<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }

    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self {
        rng.gen_range(range.0..range.1)
    }

    fn from_rot(block: m128i) -> Self {
        get_i64_from_m128i_s(block) as u64
    }

    fn from_bool(b: bool) -> Self {
        b as Self
    }

    fn modulo_2_power(self, bit_length: usize) -> Self {
        self & ((1 << bit_length) - 1)
    }
}

impl UInt for u8 {
    const NUM_BITS: usize = u8::BITS as usize;

    fn rand<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }

    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self {
        rng.gen_range(range.0..range.1)
    }

    fn from_rot(block: m128i) -> Self {
        (get_i32_from_m128i_s(block) & 0xff) as u8
    }

    fn from_bool(b: bool) -> Self {
        b as Self
    }

    fn modulo_2_power(self, bit_length: usize) -> Self {
        self & ((1 << bit_length) - 1)
    }
}

impl UInt for u128{
    const NUM_BITS: usize = u128::BITS as usize;

    fn rand<R: Rng>(rng: &mut R) -> Self {
        rng.gen()
    }

    fn rand_range<R: Rng>(rng: &mut R, range: (Self, Self)) -> Self {
        rng.gen_range(range.0..range.1)
    }

    fn from_rot(block: m128i) -> Self {
        block.into()
    }

    fn from_bool(b: bool) -> Self {
        b as Self
    }

    fn modulo_2_power(self, bit_length: usize) -> Self {
        self & ((1 << bit_length) - 1)
    }
}
