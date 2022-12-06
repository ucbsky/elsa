//! Square Correlation
use crate::{uint::UInt, ALICE};
use bytemuck::{Pod, Zeroable};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use serialize::{AsUseCast, Communicate, UseCast};

#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
/// Square Correlation on a SPDZ2k ring
pub struct SquareCorr<T: UInt>(pub [T; 2]);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SquareCorrShare<T: UInt>(pub [T; 2]);

impl<T: UInt> SquareCorr<T> {
    #[inline]
    pub fn new(value: T) -> Self {
        [value, value.wrapping_mul(&value)].into()
    }

    #[inline]
    pub fn rand<R: Rng>(rng: &mut R) -> Self {
        let a = T::rand(rng);
        let a_squared = a.wrapping_mul(&a);
        [a, a_squared].into()
    }

    #[inline]
    pub fn value(&self) -> T {
        self.0[0]
    }

    #[inline]
    pub fn value_squared(&self) -> T {
        self.0[1]
    }

    #[inline]
    /// Compute the arithmetic shares of `self`
    pub fn to_shares<R: Rng>(&self, rng: &mut R) -> (SquareCorrShare<T>, SquareCorrShare<T>) {
        let (a0, a1) = self.value().arith_shares(rng);
        let (b0, b1) = self.value_squared().arith_shares(rng);
        (SquareCorrShare([a0, b0]), SquareCorrShare([a1, b1]))
    }
}

impl<T: UInt> SquareCorrShare<T> {
    #[inline]
    pub fn a(&self) -> T {
        self.0[0]
    }

    #[inline]
    pub fn c(&self) -> T {
        self.0[1]
    }

    #[inline]
    pub fn sample_odd_t<R: Rng>(shared_rng: &mut R) -> T {
        T::rand(shared_rng) | T::one()
    }

    /// open d = ta - a' where a' is the sacrificed correlation.
    /// This function returns a share of `d`
    #[inline]
    pub fn open_d(&self, t: T, sacrificed: &Self) -> T {
        t.wrapping_mul(&self.a()).wrapping_sub(&sacrificed.a())
    }

    /// Open `w` = te - e', and this function returns a share of `w`. e is the
    /// error term of square correlation (c - a^2), and e' is the error term of
    /// sacrificed correlation (c' - a'^2)
    ///
    /// Alice send t^2c_0 - c'_0 - 2tda_0 + d^2
    /// Bob send t^2c_1 - c'_1 - 2tda_1
    #[inline]
    pub fn open_w<const PARTY: bool>(&self, t: T, sacrificed: &Self, d: T) -> T {
        let t1 = t
            .wrapping_mul(&t)
            .wrapping_mul(&self.c())
            .wrapping_sub(&sacrificed.c()); // t^2c - c'
        let t2 = t.wrapping_mul(&d).wrapping_mul(&self.a()); // tda
        let t2 = t2.wrapping_add(&t2); // 2tda

        // this branch will be optimized out
        if PARTY == ALICE {
            t1.wrapping_sub(&t2).wrapping_add(&d.wrapping_mul(&d))
        } else {
            t1.wrapping_sub(&t2) // 2tda
        }
    }

    /// Verify correctness of `correlations` using `sacrificed` correlations.
    ///
    /// #Phase 1
    /// ## Input:
    /// * `t`: public randomness
    /// ## Output:
    /// * `d_b`: a share of `ta - a'`
    /// ## Next Step:
    /// exchange `d_b` to open `d`, and go to phase 2.
    pub fn verify_phase_1(correlations: &[Self], sacrificed: &[Self], t: &[T], db_dest: &mut [T]) {
        assert_eq!(correlations.len(), db_dest.len());
        assert_eq!(correlations.len(), sacrificed.len());
        assert_eq!(correlations.len(), t.len());

        for i in 0..correlations.len() {
            let d = correlations[i].open_d(t[i], &sacrificed[i]);
            db_dest[i] = d;
        }
    }

    /// Verify correctness of `correlations` using `sacrificed` correlations.
    ///
    /// # Phase 2
    /// ## Input:
    /// * `t`: public randomness
    /// * `d`: `ta - a'`
    /// ## Output:
    /// * `w_b`: a share of `te - e'`
    /// ## Next Step:
    /// exchange `w_b` to open `w`, and check `w` is zero.
    pub fn verify_phase_2<const PARTY: bool>(
        correlations: &[Self],
        sacrificed: &[Self],
        t: &[T],
        d: &[T],
        w_dest: &mut [T],
    ) {
        assert_eq!(correlations.len(), w_dest.len());
        assert_eq!(correlations.len(), sacrificed.len());
        assert_eq!(correlations.len(), t.len());
        assert_eq!(correlations.len(), d.len());

        for i in 0..correlations.len() {
            let w = correlations[i].open_w::<PARTY>(t[i], &sacrificed[i], d[i]);
            w_dest[i] = w;
        }
    }

    #[inline]
    pub fn cut<T2: UInt>(self) -> SquareCorrShare<T2> {
        let [a, c] = self.0;
        SquareCorrShare([a.as_uint(), c.as_uint()])
    }
}

impl<T: UInt> From<[T; 2]> for SquareCorr<T> {
    fn from(value: [T; 2]) -> Self {
        SquareCorr(value)
    }
}

unsafe impl<T: UInt> Zeroable for SquareCorr<T> {}

unsafe impl<T: UInt> Pod for SquareCorr<T> {}

unsafe impl<T: UInt> Zeroable for SquareCorrShare<T> {}

unsafe impl<T: UInt> Pod for SquareCorrShare<T> {}

#[derive(Debug, Clone, Copy)]
pub struct CorrShareSeedToAlice {
    pub a_seed: u64,
    pub c_seed: u64,
}

impl CorrShareSeedToAlice {
    pub fn expand<T: UInt>(&self, size: usize) -> Vec<SquareCorrShare<T>> {
        let mut rng_a = ChaCha12Rng::seed_from_u64(self.a_seed);
        let mut rng_c = ChaCha12Rng::seed_from_u64(self.c_seed);
        (0..size)
            .map(|_| {
                let (a, c) = (T::rand(&mut rng_a), T::rand(&mut rng_c));
                SquareCorrShare([a, c])
            })
            .collect()
    }
}

impl Communicate for CorrShareSeedToAlice {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        self.a_seed.use_cast().size_in_bytes() * 2
    }

    fn to_bytes<W: std::io::Write>(&self, mut dest: W) {
        self.a_seed.use_cast().to_bytes(&mut dest);
        self.c_seed.use_cast().to_bytes(dest);
    }

    fn from_bytes<R: std::io::Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
        let a_seed = UseCast::<u64>::from_bytes(&mut bytes)?;
        let c_seed = UseCast::<u64>::from_bytes(bytes)?;
        Ok(CorrShareSeedToAlice { a_seed, c_seed })
    }
}

#[derive(Debug, Clone)]
pub struct CorrShareSeedToBob<T: UInt> {
    pub a_seed: u64,
    pub c: Vec<T>,
}

impl<T: UInt> CorrShareSeedToBob<T> {
    pub fn expand(&self) -> Vec<SquareCorrShare<T>> {
        let mut rng_a = ChaCha12Rng::seed_from_u64(self.a_seed);
        self.c
            .iter()
            .map(|c| {
                let a = T::rand(&mut rng_a);
                SquareCorrShare([a, *c])
            })
            .collect()
    }
}

impl<T: UInt> Communicate for CorrShareSeedToBob<T> {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        self.a_seed.use_cast().size_in_bytes() + self.c.size_in_bytes()
    }

    fn to_bytes<W: std::io::Write>(&self, mut dest: W) {
        self.a_seed.use_cast().to_bytes(&mut dest);
        self.c.to_bytes(dest);
    }

    fn from_bytes<R: std::io::Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
        let a_seed = UseCast::<u64>::from_bytes(&mut bytes)?;
        let c_seed = Vec::<T>::from_bytes(bytes)?;
        Ok(CorrShareSeedToBob { a_seed, c: c_seed })
    }
}

/// Create new correlation shares with size
pub fn batch_make_sqcorr_shares<T: UInt, R: Rng>(
    rng: &mut R,
    size: usize,
) -> (
    CorrShareSeedToAlice,
    CorrShareSeedToBob<T>,
    Vec<SquareCorrShare<T>>,
    Vec<SquareCorrShare<T>>,
) {
    let a0_seed = rng.next_u64();
    let a1_seed = rng.next_u64();
    let c0_seed = rng.next_u64();
    let mut a0_rng = ChaCha12Rng::seed_from_u64(a0_seed);
    let mut a1_rng = ChaCha12Rng::seed_from_u64(a1_seed);
    let mut c0_rng = ChaCha12Rng::seed_from_u64(c0_seed);
    let a0c0 = (0..size)
        .map(|_| {
            let a = T::rand(&mut a0_rng);
            let c = T::rand(&mut c0_rng);
            SquareCorrShare([a, c])
        })
        .collect::<Vec<_>>();
    let (c1, a1c1) = a0c0
        .iter()
        .map(|SquareCorrShare([a0, c0])| {
            let a1 = T::rand(&mut a1_rng);
            let a = a0.wrapping_add(&a1);
            let c = a.wrapping_mul(&a);
            let c1 = c.wrapping_sub(c0);
            (c1, SquareCorrShare([a1, c1]))
        })
        .unzip::<_, _, Vec<_>, Vec<_>>();
    (
        CorrShareSeedToAlice {
            a_seed: a0_seed,
            c_seed: c0_seed,
        },
        CorrShareSeedToBob {
            a_seed: a1_seed,
            c: c1,
        },
        a0c0,
        a1c1,
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        square_corr::{batch_make_sqcorr_shares, SquareCorrShare},
        uint::UInt,
        ALICE, BOB,
    };
    use rand::{rngs::StdRng, SeedableRng};

    fn correlations_template<T: UInt>() {
        const SIZE: usize = 1000;
        let mut rng = StdRng::seed_from_u64(12345);

        let (corr_0, corr_1, corr_0e, corr_1e) = batch_make_sqcorr_shares(&mut rng, SIZE);
        let (corr_0, corr_1) = (corr_0.expand::<T>(SIZE), corr_1.expand());
        assert_eq!(corr_0, corr_0e);
        assert_eq!(corr_1, corr_1e);
        let (sacr_0, sacr_1, sacr_0e, sacr_1e) = batch_make_sqcorr_shares(&mut rng, SIZE);
        let (sacr_0, sacr_1) = (sacr_0.expand(SIZE), sacr_1.expand());
        assert_eq!(sacr_0, sacr_0e);
        assert_eq!(sacr_1, sacr_1e);

        // check valid correlation share
        for (SquareCorrShare([a0, c0]), SquareCorrShare([a1, c1])) in corr_0
            .iter()
            .chain(sacr_0.iter())
            .zip(corr_1.iter().chain(sacr_1.iter()))
        {
            let a = a0.wrapping_add(&a1);
            let c = c0.wrapping_add(&c1);
            let a_sq = a.wrapping_mul(&a);
            assert_eq!(a_sq, c);
        }

        let t = (0..SIZE).map(|_| T::rand(&mut rng)).collect::<Vec<_>>();

        let mut d0 = vec![T::zero(); SIZE];
        let mut d1 = vec![T::zero(); SIZE];

        SquareCorrShare::verify_phase_1(&corr_0, &sacr_0, &t, &mut d0);
        SquareCorrShare::verify_phase_1(&corr_1, &sacr_1, &t, &mut d1);

        let d = d0
            .iter()
            .zip(d1.iter())
            .map(|(d0, d1)| d0.wrapping_add(d1))
            .collect::<Vec<_>>();

        let mut w0 = vec![T::zero(); SIZE];
        let mut w1 = vec![T::zero(); SIZE];

        SquareCorrShare::verify_phase_2::<{ ALICE }>(&corr_0, &sacr_0, &t, &d, &mut w0);
        SquareCorrShare::verify_phase_2::<{ BOB }>(&corr_1, &sacr_1, &t, &d, &mut w1);

        let w = w0
            .iter()
            .zip(w1.iter())
            .map(|(w0, w1)| w0.wrapping_add(w1))
            .collect::<Vec<_>>();

        // check w is all zero
        for w in w.iter() {
            assert_eq!(w, &T::zero());
        }
    }

    #[test]
    fn correlation_u128() {
        correlations_template::<u128>();
    }
}
