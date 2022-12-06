//! # Simulation Procedure
//! # Clients (for quantization only)
//! * Generate some garbage `y`(8-bit) and `s` (8-bit)
//! * Generate some COT `hsize * (gsize / ??) + wsize * gsize + wsize * (gsize / ??)`
//!
//! # Client (more serious one)
//! not needed. on server, generate dummy `y`, `s` and input shares
//! # Server MPC
//! * Perform `(hsize - 1) * gsize + gsize * wsize` AND gate for each client if not optimized
//!   OR `(hsize - 1) * (gsize / ??) + (gsize / ??) * wsize` AND gate for each client if optimized
//! * Run `one_hot_filter` on dummy value `s`
//! * Run `decode` on dummy value `y` and `s`
//! * Run B2A MPC and dummy input shares (gsize / 2 * wsize) `wsize = 32`

use crate::{uint::UInt};



/// `bit_mul` returns arithmetic share or `x0 & x1`.
/// * `j`: ring size to operate on (2^j)
/// * `x0`: a share of `x`.
/// * `v0`: trimmed rot `H(q)`
/// * `v1`: trimmed rot `H(q + delta)`
///
/// This function returns:
/// * `y0`, such that `y0 + y1 mod 2^j = x0 & x1`
/// * `u`, such that `u = v0 + v1 + x0 mod 2^j`
pub fn bit_mul_as_ot_sender<T: UInt>(j: usize, x0: bool, v0: T, v1: T) -> (T, T) {
    // treat `x0` as a wrapped u32
    let x0 = T::from_bool(x0);

    let y0 = v0.wrapping_neg().modulo_2_power(j);
    let u = v0.wrapping_add(&v1).wrapping_add(&x0).modulo_2_power(j);

    (y0, u)
}

/// `bit_mul` returns arithmetic share or `a0 & b1`.
/// * `a0`: a share on my side
/// * `v0`: trimmed rot `H(q)`
/// * `v1`: trimmed rot `H(q + delta)`
///
/// This function returns:
/// * `y0`, such that `y0 ^ y1 = a0 & b1`
/// * `u`, such that `u = v0 ^ v1 ^ a0`
pub fn bit_mul_bool_as_ot_sender<T: UInt>(a0: bool, v0: T, v1: T) -> (bool, bool) {
    // treat `x0` as a wrapped u32
    let x0 = T::from_bool(a0);

    let y0 = (v0.wrapping_neg() & T::one()) == T::one();
    let u = v0.wrapping_add(&v1).wrapping_add(&x0) & T::one() == T::one();

    (y0, u)
}

/// `bit_mul` returns arithmetic share or `x0 & x1`.
/// * `b1`: a share on my side
/// * `v`: trimmed rot `H(q + select_bit * delta)`
///
/// Returns:
/// * `y1`, such that `y0 + y1 mod 2^j = x0 & x1`
pub fn bit_mul_as_ot_receiver<T: UInt>(j: usize, x1: bool, v: T, u: T) -> T {
    if x1 {
        // v = v1
        // y = x0 because x1 = 1
        // y0 = -v0
        // y1 = u - v = v0 + v1 + x0 - v1 = v0 + x0
        u.wrapping_sub(&v).modulo_2_power(j)
    } else {
        // v = v0
        v.modulo_2_power(j)
    }
}

/// `bit_mul` returns boolean share or `a0 & b1`.
/// * `b1`: a share on my side
/// * `v`: trimmed rot `H(q + select_bit * delta)`
///
/// Returns:
/// * `y1`, such that `y0 ^ y1 = a0 & b1`
pub fn bit_mul_bool_as_ot_receiver<T: UInt>(b1: bool, v_selected: T, u: bool) -> bool {
    // just extract first bit
    let v = v_selected & T::one() == T::one();
    if b1 {
        // v = v1
        u ^ v
    } else {
        // v = v0
        v
    }
}

pub trait AndGate {
    /// `x`: a share of `x`.
    /// `y`: a share of `y`.
    ///
    ///  # Selected Bits
    /// Those bits are selected bits for COT generation.
    /// `[y, x]`
    ///
    /// return: a share of `x & y`.
    fn and(&mut self, x: bool, y: bool) -> bool;
}

///  `(x AND y) =  (x_0 XOR x_1) AND (y_0 XOR y_1) = (x_0 AND y_0) XOR (x_1 AND
/// y_1) XOR (x_0 AND y_1) XOR (y_0 AND x_1)`. Notice that first 2 terms are
/// local for parties to compute (server 0 can compute 1st, server 1 can compute
/// 2nd). The remaining last 2 terms can be computed with 2 calls to BitMult.
pub struct AndGateUsingOTSender<'a, T: UInt> {
    v0s: &'a [T],
    v1s: &'a [T],
    us: Vec<bool>,
    pos: usize,
}

impl<'a, T: UInt> AndGateUsingOTSender<'a, T> {
    pub fn new(v0s: &'a [T], v1s: &'a [T]) -> Self {
        AndGateUsingOTSender {
            v0s,
            v1s,
            us: Vec::new(),
            pos: 0,
        }
    }
    #[must_use]
    pub fn done_and_get_us(self) -> Vec<bool> {
        self.us
    }
}

impl<'a, T: UInt> AndGate for AndGateUsingOTSender<'a, T> {
    fn and(&mut self, x0: bool, y0: bool) -> bool {
        let x0y0 = x0 & y0;
        let (x0y10, u0) = bit_mul_bool_as_ot_sender(x0, self.v0s[self.pos], self.v1s[self.pos]);
        self.pos += 1;
        let (y0x10, u1) = bit_mul_bool_as_ot_sender(y0, self.v0s[self.pos], self.v1s[self.pos]);
        self.pos += 1;
        self.us.push(u0);
        self.us.push(u1);

        x0y0 ^ x0y10 ^ y0x10
    }
}

pub struct AndGateUsingOTReceiver<'a, T: UInt> {
    // selected bits should be y1, x1. in order
    v_selected: &'a [T],
    us: &'a [bool],
    pos: usize,
}

impl<'a, T: UInt> AndGateUsingOTReceiver<'a, T> {
    pub fn new(v_selected: &'a [T], us: &'a [bool]) -> Self {
        AndGateUsingOTReceiver {
            v_selected,
            us,
            pos: 0,
        }
    }
}

impl<'a, T: UInt> AndGate for AndGateUsingOTReceiver<'a, T> {
    fn and(&mut self, x1: bool, y1: bool) -> bool {
        let x1y1 = x1 & y1;
        let x0y11 = bit_mul_bool_as_ot_receiver(y1, self.v_selected[self.pos], self.us[self.pos]);
        self.pos += 1;
        let y0x11 = bit_mul_bool_as_ot_receiver(x1, self.v_selected[self.pos], self.us[self.pos]);
        self.pos += 1;
        x1y1 ^ x0y11 ^ y0x11
    }
}

// /// Simulation AND gate for OT receiver, for clients to generate selected bits.
// pub struct SimulationAndGateForSelectedBits<'a, T: UInt> {
//     v0s: &'a [T],
//     v1s: &'a [T],
//     pos: usize,
// }

/// A dummy AND gate of boolean shares, which is incorrect, but useful for
/// profiling.
pub struct DummyAndGate;

impl DummyAndGate {

}

impl AndGate for DummyAndGate {
    fn and(&mut self, _x: bool, _y: bool) -> bool {
        false
    }
}

/// This requires us to run AndGate on Alice side first, and then Bob side.
pub struct LocalAndGateForAlice {
    x0s: Vec<bool>,
    y0s: Vec<bool>,
}

pub struct LocalAndGateForBob {
    alice_data: LocalAndGateForAlice,
    pos: usize,
}

impl LocalAndGateForAlice {
    pub fn new() -> Self {
        LocalAndGateForAlice {
            x0s: Vec::new(),
            y0s: Vec::new(),
        }
    }

    pub fn into_bob_and_gate(self) -> LocalAndGateForBob {
        LocalAndGateForBob {
            alice_data: self,
            pos: 0,
        }
    }
}

impl AndGate for LocalAndGateForAlice {
    fn and(&mut self, x0: bool, y0: bool) -> bool {
        self.x0s.push(x0);
        self.y0s.push(y0);
        false
    }
}

impl AndGate for LocalAndGateForBob {
    fn and(&mut self, x1: bool, y1: bool) -> bool {
        let x0 = self.alice_data.x0s[self.pos];
        let y0 = self.alice_data.y0s[self.pos];
        self.pos += 1;
        (x0 ^ x1) & (y0 ^ y1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cot::{
        client::COTGen,
        rot::{cot_to_rot_receiver_side, cot_to_rot_sender_side},
    };
    use itertools::Itertools;
    use rand::{rngs::StdRng, SeedableRng};
    use crate::bits::PackedBits;

    #[test]
    fn test_local_and_gate() {
        let mut rng = StdRng::seed_from_u64(12345);
        let x = PackedBits::rand(&mut rng, 100);
        let x0 = PackedBits::rand(&mut rng, 100);
        let x1 = &x ^ &x0;

        let y = PackedBits::rand(&mut rng, 100);
        let y0 = PackedBits::rand(&mut rng, 100);
        let y1 = &y ^ &y0;

        let mut alice = LocalAndGateForAlice::new();
        let xy0 = x0
            .iter()
            .zip(y0.iter())
            .map(|(x0, y0)| alice.and(x0, y0))
            .collect::<PackedBits>();
        let mut bob = alice.into_bob_and_gate();
        let xy1 = x1
            .iter()
            .zip(y1.iter())
            .map(|(x1, y1)| bob.and(x1, y1))
            .collect::<PackedBits>();

        let xy_expected = &x & &y;
        let xy_actual = &xy0 ^ &xy1;

        assert_eq!(xy_expected, xy_actual);
    }

    #[test]
    fn test_bitmul_bool() {
        let mut rng = StdRng::seed_from_u64(12345);
        let x0s = PackedBits::rand(&mut rng, 128);
        let x1s = PackedBits::rand(&mut rng, 128);
        let delta = COTGen::sample_delta(&mut rng);

        let (client_sender_msg, client_receiver_msg) =
            COTGen::sample_cots_using_selected_bits(&mut rng, x1s.iter(), x1s.len(), delta, 128);
        let (v0s, v1s) =
            cot_to_rot_sender_side::<u32>(&client_sender_msg.qs_seed.expand(x1s.len()), delta);
        let v_selected = cot_to_rot_receiver_side::<u32>(&client_receiver_msg.ts);

        let (x0x10s, us) = x0s
            .iter()
            .zip(v0s)
            .zip(v1s)
            .map(|((x0, v0), v1)| bit_mul_bool_as_ot_sender(x0, v0, v1))
            .unzip::<_, _, Vec<_>, Vec<_>>();
        let x0x10s = x0x10s.into_iter().collect::<PackedBits>();

        let x0x11s = x1s
            .iter()
            .zip(v_selected)
            .zip(us)
            .map(|((x1, v), u)| bit_mul_bool_as_ot_receiver(x1, v, u)).collect::<PackedBits>();

        let x0x1_expected = &x0s & &x1s;
        let x0x1_actual = &x0x10s ^ &x0x11s;

        assert_eq!(x0x1_expected, x0x1_actual);
    }

    #[test]
    fn test_ot_and_gate() {
        let mut rng = StdRng::seed_from_u64(12345);
        let xs = PackedBits::rand(&mut rng, 100);
        let x0s = PackedBits::rand(&mut rng, 100);
        let x1s = &xs ^ &x0s;

        let ys = PackedBits::rand(&mut rng, 100);
        let y0s = PackedBits::rand(&mut rng, 100);
        let y1s = &ys ^ &y0s;

        let selected_bits = y1s.iter().interleave(x1s.iter());

        let delta = COTGen::sample_delta(&mut rng);
        let num_ots = x1s.len() * 2;
        let (client_sender_msg, client_receiver_msg) = COTGen::sample_cots_using_selected_bits(
            &mut rng,
            selected_bits,
            x1s.len() * 2,
            delta,
            128,
        );

        // alice
        let qs = client_sender_msg.qs_seed.expand(num_ots);
        let (v0s, v1s) = cot_to_rot_sender_side::<u32>(&qs, delta);
        let mut alice = AndGateUsingOTSender::new(&v0s, &v1s);
        let xy0 = x0s
            .iter()
            .zip(y0s.iter())
            .map(|(x0, y0)| alice.and(x0, y0))
            .collect::<PackedBits>();
        let us = alice.done_and_get_us();

        // bob
        let cot_selected = client_receiver_msg.ts;
        let v_selected = cot_to_rot_receiver_side::<u32>(&cot_selected);
        let mut bob = AndGateUsingOTReceiver::new(&v_selected, &us);
        let xy1 = x1s
            .iter()
            .zip(y1s.iter())
            .map(|(x1, y1)| bob.and(x1, y1))
            .collect::<PackedBits>();

        let xy_expected = &xs & &ys;
        let xy_actual = &xy0 ^ &xy1;

        assert_eq!(xy_expected, xy_actual);
    }
}
