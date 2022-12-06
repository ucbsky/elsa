//! This module contains the A2S (Arithmetic Share to Arithmetic Share of
//! Squares) protocol implementation.

use crate::{square_corr::SquareCorrShare, uint::UInt, ALICE};

/// First round of A2S: open `x-a`
/// `xb`: arithmetic share of the `x`
/// `corr_b`: share of square correction
///
/// # Returns
/// Share of `x-a`
#[inline]
pub fn a2s_first<C: UInt>(xb: C, corr_b: SquareCorrShare<C>) -> C {
    xb.wrapping_sub(&corr_b.a())
}

/// Batch version of `a2s_first`
/// `xbs`: arithmetic shares of `x`
/// `corr_bs`: square correction shares
///
/// # Returns
/// Batch of `x-a` shares
#[inline]
pub fn batch_a2s_first<C: UInt>(xbs: &[C], corr_bs: &[SquareCorrShare<C>]) -> Vec<C> {
    xbs.iter()
        .zip(corr_bs.iter())
        .map(|(xb, corr_b)| a2s_first(*xb, *corr_b))
        .collect()
}

/// Second round of A2S
/// `e`: `x-a`
/// `xb`: arithmetic share of the `x`
/// `corr_b`: share of square correction. This correlation should be the same as
/// in the first round.
///
/// # Returns
/// Share of `x^2`
#[inline]
pub fn a2s_second<T: UInt, const PARTY: bool>(e: T, xb: T, corr_b: SquareCorrShare<T>) -> T {
    let e_doubled = e.wrapping_add(&e);
    let cb = corr_b.c();
    // t1 = 2ex_b + c_b
    let t1 = e_doubled.wrapping_mul(&xb).wrapping_add(&cb);
    if PARTY == ALICE {
        let e_squared = e.wrapping_mul(&e);
        t1.wrapping_sub(&e_squared)
    } else {
        t1
    }
}

/// Batch version of `a2s_second`
/// `es`: `x-a` batch
/// `xbs`: arithmetic shares of `x`
/// `corr_bs`: square correction shares
///
/// # Returns
/// Batch of `x^2` shares
#[inline]
pub fn batch_a2s_second<C: UInt, const PARTY: bool>(
    es: &[C],
    xbs: &[C],
    corr_bs: &[SquareCorrShare<C>],
) -> Vec<C> {
    es.iter()
        .zip(xbs.iter())
        .zip(corr_bs.iter())
        .map(|((e, xb), corr_b)| a2s_second::<_, PARTY>(*e, *xb, *corr_b))
        .collect()
}

#[cfg(test)]
mod test {
    use crate::{
        a2s::{batch_a2s_first, batch_a2s_second},
        square_corr::SquareCorr,
        uint::UInt,
        ALICE, BOB,
    };
    use rand::{rngs::StdRng, SeedableRng};

    fn a2s_for_type<T: UInt, CORR: UInt, const GSIZE: usize>() {
        let mut rng = StdRng::seed_from_u64(12345);
        let x = (0..GSIZE).map(|_| T::rand(&mut rng)).collect::<Vec<_>>();
        let (x0, x1) = x
            .iter()
            .map(|x| x.arith_shares(&mut rng))
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let corr = (0..GSIZE)
            .map(|_| SquareCorr::<CORR>::rand(&mut rng))
            .collect::<Vec<_>>();
        let (corr_0, corr_1) = corr
            .iter()
            .map(|c| c.to_shares(&mut rng))
            .unzip::<_, _, Vec<_>, Vec<_>>();

        // cut the correlation to smaller values
        let corr_0 = corr_0.iter().map(|c| c.cut()).collect::<Vec<_>>();
        let corr_1 = corr_1.iter().map(|c| c.cut()).collect::<Vec<_>>();

        let e0 = batch_a2s_first(&x0, &corr_0);
        let e1 = batch_a2s_first(&x1, &corr_1);

        let e = e0
            .iter()
            .zip(e1.iter())
            .map(|(x0, x1)| x0.wrapping_add(x1))
            .collect::<Vec<_>>();

        let x_sq0 = batch_a2s_second::<_, { ALICE }>(&e, &x0, &corr_0);
        let x_sq1 = batch_a2s_second::<_, { BOB }>(&e, &x1, &corr_1);

        let x_sq_expected = x.iter().map(|x| x.wrapping_mul(x)).collect::<Vec<_>>();
        let s_sq_actual = x_sq0
            .iter()
            .zip(x_sq1.iter())
            .map(|(x0, x1)| x0.wrapping_add(x1))
            .collect::<Vec<_>>();

        assert_eq!(x_sq_expected, s_sq_actual);
    }

    #[test]
    fn a2s() {
        a2s_for_type::<u8, u16, 1000>();
        a2s_for_type::<u16, u32, 1000>();
        a2s_for_type::<u32, u128, 1000>();
        a2s_for_type::<u64, u128, 1000>();
    }
}
