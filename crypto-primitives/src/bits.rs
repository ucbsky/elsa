use crate::uint::UInt;
use bytemuck::{Pod, Zeroable};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::{
    borrow::Borrow,
    fmt::{Debug, Display, Formatter},
    iter::FromIterator,
    ops::{BitAnd, BitXor, Not},
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
/// Individual bits of a `UInt` in little endian order.
pub struct BitsLE<T: UInt>(pub T);

impl<T: UInt> Display for BitsLE<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl<T: UInt> AsRef<T> for BitsLE<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

unsafe impl<T: UInt> Zeroable for BitsLE<T> {}

unsafe impl<T: UInt> Pod for BitsLE<T> {}

impl<T: UInt> BitsLE<T> {
    /// Get bit at index `i` in little endian order.
    ///
    /// # Panics
    /// Panic if `i >= T::NUM_BITS`.
    pub fn get_bit(self, i: usize) -> bool {
        let me = self.0;
        (me >> i) & T::one() == T::one()
    }

    /// Set bit at index `i` in little endian order.
    ///
    /// # Panics
    /// Panic if `i >= T::NUM_BITS`.
    #[must_use]
    pub fn set_bit(self, i: usize, bit: bool) -> Self {
        let me = self.0;
        let mask = T::one() << i;
        let c_mask = T::from_bool(bit) << i;
        BitsLE((me & !mask) | c_mask)
    }

    /// Return an iterator over the bits in little endian order.
    pub fn iter(self) -> impl Iterator<Item = bool> {
        (0..T::NUM_BITS).map(move |i| self.get_bit(i))
    }

    /// Get a boolean share of `self` such that `share_0 ^ share_1 = self`.
    pub fn to_boolean_shares<R: Rng>(self, rng: &mut R) -> (Self, Self) {
        let share0 = T::rand(rng).bits_le();
        let share1 = self ^ share0;
        (share0, share1)
    }

    /// Get `self` from boolean slices.
    ///
    /// # Panics (Debug only)
    /// Panic if `booleans.len() > T::NUM_BITS`.
    pub fn from_booleans(booleans: &[bool]) -> Self {
        debug_assert!(
            booleans.len() <= T::NUM_BITS,
            "booleans.len() = {}, T::NUM_BITS = {}",
            booleans.len(),
            T::NUM_BITS
        );
        booleans
            .iter()
            .enumerate()
            .map(|(shift, b)| if *b { T::one() << shift } else { T::zero() })
            .sum::<T>()
            .bits_le()
    }

    /// Return number representation of `self`.
    pub fn arith(self) -> T {
        self.0
    }

    /// Number of bits.
    pub fn len(self) -> usize {
        T::NUM_BITS
    }

    pub fn is_empty(self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct SeededInputShare(pub u64);

impl SeededInputShare {
    pub fn expand<T: UInt>(self, size: usize) -> Vec<BitsLE<T>> {
        let mut rng = ChaCha12Rng::seed_from_u64(self.0);
        (0..size).map(|_| BitsLE(T::rand(&mut rng))).collect()
    }
}

unsafe impl Pod for SeededInputShare {}
unsafe impl Zeroable for SeededInputShare {}

/// Return `inputs_0` as PRNG seed, and `inputs_1`.
pub fn batch_make_boolean_shares<T: UInt, R: Rng, I>(
    rng: &mut R,
    input: I,
) -> (SeededInputShare, Vec<BitsLE<T>>)
where
    I: Iterator,
    I::Item: Borrow<BitsLE<T>>,
{
    let seed = rng.next_u64();
    let mut rng = ChaCha12Rng::seed_from_u64(seed);
    let inputs_1 = input
        .map(|b| {
            let mask = T::rand(&mut rng).bits_le();
            *b.borrow() ^ mask
        })
        .collect::<Vec<_>>();
    (SeededInputShare(seed), inputs_1)
}

impl<T: UInt> BitXor for BitsLE<T> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        (self.0 ^ rhs.0).bits_le()
    }
}

impl<T: UInt> BitAnd for BitsLE<T> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        (self.0 & rhs.0).bits_le()
    }
}

impl<T: UInt> Not for BitsLE<T> {
    type Output = Self;

    fn not(self) -> Self::Output {
        (!self.0).bits_le()
    }
}

/// Stores bits in packed form.
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct PackedBits {
    // number of bits
    size: usize,
    payload: Vec<BitsLE<u32>>,
}

impl PackedBits {
    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Iterate over bits.
    pub fn iter(&self) -> impl Iterator<Item = bool> + '_ {
        self.payload
            .iter()
            .flat_map(|bits| bits.iter())
            .take(self.size)
    }

    pub fn rand<R: Rng>(rng: &mut R, num_bits: usize) -> Self {
        let num_u32 = (num_bits + 31) >> 5;
        let payload = (0..num_u32)
            .map(|_| BitsLE(rng.gen::<u32>()))
            .collect::<Vec<_>>();

        let mut result = Self {
            size: num_bits,
            payload,
        };

        result.adjust_last_byte();
        result
    }

    fn adjust_last_byte(&mut self) {
        let num_bits = self.size;

        let offset = num_bits % 32;
        if offset != 0 {
            let mask = (1u32 << offset) - 1;
            self.payload.last_mut().unwrap().0 &= mask;
        }
    }

    pub fn to_boolean_shares<R: Rng>(&self, rng: &mut R) -> (Self, Self) {
        let (share0, share1) = self
            .payload
            .iter()
            .map(|bits| bits.to_boolean_shares(rng))
            .unzip();
        let mut share0 = Self {
            size: self.size,
            payload: share0,
        };
        share0.adjust_last_byte();
        let mut share1 = Self {
            size: self.size,
            payload: share1,
        };
        share1.adjust_last_byte();
        (share0, share1)
    }
}

impl BitAnd for &PackedBits {
    type Output = PackedBits;

    fn bitand(self, rhs: Self) -> Self::Output {
        let payload = self
            .payload
            .iter()
            .zip(rhs.payload.iter())
            .map(|(a, b)| *a & *b)
            .collect::<Vec<_>>();
        PackedBits {
            size: self.size,
            payload,
        }
    }
}

impl BitXor for &PackedBits {
    type Output = PackedBits;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let payload = self
            .payload
            .iter()
            .zip(rhs.payload.iter())
            .map(|(a, b)| *a ^ *b)
            .collect::<Vec<_>>();
        PackedBits {
            size: self.size,
            payload,
        }
    }
}

impl Not for &PackedBits {
    type Output = PackedBits;

    fn not(self) -> Self::Output {
        let payload = self.payload.iter().map(|&bits| !bits).collect::<Vec<_>>();

        let mut result = PackedBits {
            size: self.size,
            payload,
        };
        result.adjust_last_byte();
        result
    }
}

impl FromIterator<bool> for PackedBits {
    fn from_iter<T: IntoIterator<Item = bool>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let mut size = 0;
        let mut payload = Vec::new();
        for bit in iter {
            if size % 32 == 0 {
                payload.push(BitsLE::zeroed());
            }
            let last = payload.last_mut().unwrap();
            *last = last.set_bit(size % 32, bit);
            size += 1;
        }
        PackedBits { size, payload }
    }
}

impl<'a> FromIterator<&'a bool> for PackedBits {
    fn from_iter<T: IntoIterator<Item = &'a bool>>(iter: T) -> Self {
        iter.into_iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{bits::PackedBits, uint::UInt, utils::SliceExt};
    use rand::{rngs::StdRng, Rng, SeedableRng};

    use super::batch_make_boolean_shares;

    #[test]
    fn test_consistency() {
        let mut rng = StdRng::seed_from_u64(12345);

        let bits = PackedBits::rand(&mut rng, 33);
        let bools = bits.iter().collect::<Vec<_>>();
        let bits2 = bools.iter().collect::<PackedBits>();
        assert_eq!(bits, bits2);
        let bools2 = bits2.iter().collect::<Vec<_>>();
        assert_eq!(bools, bools2);

        let bools3 = (0..55).map(|_| rng.gen()).collect::<Vec<bool>>();
        let bits3 = bools3.iter().collect::<PackedBits>();
        let bools4 = bits3.iter().collect::<Vec<_>>();
        assert_eq!(bools3, bools4);
    }

    #[test]
    fn test_consistency_2() {
        let v1 = vec![false, false, true, false, true, false, true, true];
        let not_v1 = vec![true, true, false, true, false, true, false, false];
        let v2 = vec![true, false, true, false, true, false, true, false];
        let v1_and_v2 = vec![false, false, true, false, true, false, true, false];

        let v1_vec = v1.iter().cloned().collect::<PackedBits>();
        let v2_vec = v2.iter().cloned().collect::<PackedBits>();
        let not_v1_vec = !&v1_vec;
        let v1_and_v2_vec = &v1_vec & &v2_vec;

        assert_eq!(not_v1_vec.iter().collect::<Vec<_>>(), not_v1);
        assert_eq!(v1_and_v2_vec.iter().collect::<Vec<_>>(), v1_and_v2);
    }

    #[test]
    fn make() {
        let mut rng = StdRng::seed_from_u64(12345);
        let gsize = 1000;
        let inputs = (0..gsize)
            .map(|_| rng.gen::<u64>().bits_le())
            .collect::<Vec<_>>();
        let (inputs_0, inputs_1) = batch_make_boolean_shares(&mut rng, inputs.iter());
        let inputs_0 = inputs_0.expand(gsize);
        let merged = inputs_0.zip_map(&inputs_1, |a, b| (*a ^ *b));
        assert_eq!(inputs, merged);
    }
}
