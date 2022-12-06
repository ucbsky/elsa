//! boolean to arithmetic operations

// client: 32-bit 10000: 32 * 128 * 10000
// server: 32 * 10000 * #num clients * 32

use crate::{
    bitmul::{bit_mul_as_ot_receiver, bit_mul_as_ot_sender},
    bits::BitsLE,
    cot::rot::{cot_to_rot_receiver_side, cot_to_rot_sender_side},
    uint::UInt,
};
use block::Block;

/// `bit_comp_as_ot_sender_single` converts boolean share of one number into
/// arithmetic share. `B` is boolean share of input ring bounded by L_infinity,
/// and `A` is arithmetic share of output ring.
/// * `x0s`: boolean share in little endian. Should have length T::NUM_BITS.
/// * `v0s`: trimmed ROT first element
/// * `v1s`: trimmed ROT second element
/// * `us_dest`: return `u` that is used by `bit_comp_as_ot_receiver_inner`
///
/// returns:
/// * `y0s` in ring `A` such that `y0s + y1s = x0s ^ x1s`
pub fn bit_comp_as_ot_sender_single<B: UInt, A: UInt>(
    x0s: BitsLE<B>,
    v0s: &[A],
    v1s: &[A],
    us_dest: &mut [A],
) -> A {
    debug_assert_eq!(x0s.len(), B::NUM_BITS);
    debug_assert_eq!(v0s.len(), B::NUM_BITS);
    debug_assert_eq!(v1s.len(), B::NUM_BITS);
    debug_assert_eq!(us_dest.len(), B::NUM_BITS);

    let mut z = A::zero();
    x0s.iter()
        .enumerate()
        .zip(v0s)
        .zip(v1s)
        .zip(us_dest)
        .for_each(|((((i, x0), v0), v1), u_dest)| {
            let lp = A::NUM_BITS - (i + 1);
            let (y0, u) = bit_mul_as_ot_sender(lp, x0, *v0, *v1);
            *u_dest = u;

            // t = x0 - 2y0
            let t = A::from_bool(x0).wrapping_sub(&y0.wrapping_add(&y0));
            // z += t * 2^i
            z = z.wrapping_add(&(t << i));
        });

    z
}

/// `bit_comp_as_ot_receiver_inner` converts boolean share of one number into
/// arithmetic share.`B` is boolean share of input ring bounded by L_infinity,
/// and `A` is arithmetic share of output ring.
/// * `x1s`: boolean share in little endian. Should have length T::NUM_BITS.
/// * `vs`:  trimmed ROT selected elements
/// * `us`: `us` sent by OT sender
///
/// returns:
/// * `y1s` such that `y0s + y1s = x0s ^ x1s`
pub fn bit_comp_as_ot_receiver_single<B: UInt, A: UInt>(x1s: BitsLE<B>, vs: &[A], us: &[A]) -> A {
    debug_assert_eq!(x1s.len(), B::NUM_BITS);
    debug_assert_eq!(vs.len(), B::NUM_BITS);
    debug_assert_eq!(us.len(), B::NUM_BITS);

    let mut z = A::zero();
    x1s.iter()
        .enumerate()
        .zip(vs)
        .zip(us)
        .for_each(|(((i, x1), t), u)| {
            let lp = A::NUM_BITS - (i + 1);
            let y1 = bit_mul_as_ot_receiver(lp, x1, *t, *u);
            let t = A::from_bool(x1).wrapping_sub(&y1.wrapping_add(&y1));

            // z += t * 2^i
            z = z.wrapping_add(&(t << i));
        });

    z
}

/// `bit_comp_as_ot_sender_batch` converts boolean share of `N` numbers into `N`
/// arithmetic shares.`B` is boolean share of input ring bounded by L_infinity,
/// and `A` is arithmetic share of output ring.
/// * `inputs_0`: boolean shares of `N` numbers in little endian. Should have
///   length `N`, and each boolean share should have length T::NUM_BITS.
/// * `delta`: COT delta.
/// * `qs`:  COT first elements. Should have length `N * T::NUM_BITS`
///
/// Returns
/// * `y0s`: `Vec<A>` of length `N` such that `y0s + y1s = x0s ^ x1s`
/// * `us`: `Vec<A>` of length `N * B::NUM_BITS` that will be sent to OT
///   receiver
///
/// # Panics
/// Panics if length requirements are not met.
pub fn bit_comp_as_ot_sender_batch<I: UInt, A: UInt>(
    inputs_0: &[BitsLE<I>],
    delta: Block,
    qs: &[Block],
) -> (Vec<A>, Vec<A>) {
    let n = inputs_0.len();

    assert_eq!(qs.len(), n * I::NUM_BITS);

    // convert COT to ROT
    let (v0s, v1s) = cot_to_rot_sender_side(qs, delta);

    let mut us_dest = vec![A::zero(); n * I::NUM_BITS];

    let y0s = inputs_0
        .iter()
        .zip(v0s.chunks(I::NUM_BITS))
        .zip(v1s.chunks(I::NUM_BITS))
        .zip(us_dest.chunks_mut(I::NUM_BITS))
        .map(|(((x0s, v0s), v1s), u_dest)| bit_comp_as_ot_sender_single(*x0s, v0s, v1s, u_dest))
        .collect();
    (y0s, us_dest)
}

/// `bit_comp_as_ot_receiver_batch` converts boolean share of `N` numbers into
/// `N` arithmetic shares. `B` is boolean share of input ring bounded by
/// L_infinity, and `A` is arithmetic share of output ring.
/// * `inputs_1`: boolean shares of `N` numbers in little endian. Should have
///   length `N`, and each boolean share should have length T::NUM_BITS.
/// * `ts`:  COT selected elements. Should have length `N * T::NUM_BITS`
/// * `us`: `us` sent by OT sender. Should have length `N * T::NUM_BITS`
///
/// Returns `Vec<T>` of length `N` such that `y0s + y1s = x0s ^ x1s`
///
/// # Panics
/// Panics if length requirements are not met.
pub fn bit_comp_as_ot_receiver_batch<B: UInt, A: UInt>(
    inputs_1: &[BitsLE<B>],
    ts: &[Block],
    us: &[A],
) -> Vec<A> {
    let n = inputs_1.len();

    assert_eq!(ts.len(), n * B::NUM_BITS);
    assert_eq!(us.len(), n * B::NUM_BITS);

    // convert COT to ROT
    let vs = cot_to_rot_receiver_side(ts);

    inputs_1
        .iter()
        .zip(vs.chunks(B::NUM_BITS))
        .zip(us.chunks(B::NUM_BITS))
        .map(|((x1s, vs), u)| bit_comp_as_ot_receiver_single(*x1s, vs, u))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bits::PackedBits,
        cot::{
            client::{num_additional_ot_needed, COTGen},
            server::{sample_chi, OTReceiver, OTSender},
        },
    };
    use rand::{rngs::StdRng, SeedableRng};
    use serialize::{AsUseCast, Communicate};

    #[test]
    fn test_bit_mul() {
        const NUM_BITS: usize = 16;
        const J: usize = 31;
        let mut rng = StdRng::seed_from_u64(12345);
        let x0s = PackedBits::rand(&mut rng, NUM_BITS);
        let x1s = PackedBits::rand(&mut rng, NUM_BITS);
        let x0s_and_x1s = &x0s & &x1s;
        let delta = COTGen::sample_delta(&mut rng);
        let qs = (0..NUM_BITS)
            .map(|_| Block::rand(&mut rng))
            .collect::<Vec<_>>();
        let ts = qs
            .iter()
            .zip(x1s.iter())
            .map(|(q, c)| if c { q.add_gf(delta) } else { *q })
            .collect::<Vec<_>>();

        let (v0s, v1s) = cot_to_rot_sender_side(&qs[..NUM_BITS], delta);
        let vs = cot_to_rot_receiver_side(&ts[..NUM_BITS]);

        let (y0s, us): (Vec<_>, Vec<_>) = x0s
            .iter()
            .zip(v0s.iter().zip(v1s.iter()))
            .map(|(x0, (v0, v1))| bit_mul_as_ot_sender::<u32>(J, x0, *v0, *v1))
            .unzip();
        let y1s = x1s
            .iter()
            .zip(vs.iter())
            .zip(us.iter())
            .map(|((x1, &v), &u)| bit_mul_as_ot_receiver(J, x1, v, u))
            .collect::<Vec<_>>();

        let ys = y0s
            .iter()
            .zip(y1s.iter())
            .map(|(&y0, &y1)| y0.wrapping_add(y1).modulo_2_power(J))
            .collect::<Vec<_>>();

        assert_eq!(ys.len(), NUM_BITS);
        let x0s_and_x1s = x0s_and_x1s.iter().map(u32::from_bool).collect::<Vec<_>>();

        assert_eq!(ys, x0s_and_x1s);
    }

    fn serialize_and_deserialize<T: Communicate>(t: T) -> T::Deserialized {
        let bytes = t.into_bytes_owned();
        T::from_bytes_owned(bytes).unwrap()
    }

    fn b2a_end_to_end_template<I: UInt, A: UInt>() {
        const GSIZE: usize = 100;
        let num_bits = GSIZE * I::NUM_BITS;
        let mut rng = StdRng::seed_from_u64(12345);

        let inputs = (0..GSIZE).map(|_| I::rand(&mut rng)).collect::<Vec<_>>();
        let (inputs_0, inputs_1) = inputs
            .iter()
            .map(|x| x.bits_le().to_boolean_shares(&mut rng))
            .unzip::<_, _, Vec<_>, Vec<_>>();
        let inputs_0 = serialize_and_deserialize(inputs_0);
        let inputs_1 = serialize_and_deserialize(inputs_1);

        let delta = COTGen::sample_delta(&mut rng);
        let delta = serialize_and_deserialize(delta.use_cast());
        let num_additional = num_additional_ot_needed(num_bits);
        let (msg_to_sender, msg_to_receiver) =
            COTGen::sample_cots(&mut rng, &inputs_1, delta, num_additional);

        let msg_to_sender = serialize_and_deserialize(msg_to_sender);
        let msg_to_receiver = serialize_and_deserialize(msg_to_receiver);

        // first round: verify
        let chi = sample_chi(num_bits + num_additional, 99999);
        // OT receiver send
        let (x_til, t_til) = OTReceiver::send_x_til_t_til(
            &msg_to_receiver.ts,
            &chi,
            &inputs_1,
            msg_to_receiver.r_seed,
        );

        // OT sender receive
        let qs = {
            let (x_til, t_til) = serialize_and_deserialize((x_til.use_cast(), t_til));
            let (qs, result) = OTSender::verify_and_get_cot(
                msg_to_sender.qs_seed,
                &chi,
                msg_to_sender.delta,
                x_til,
                t_til,
            );
            assert!(result);
            qs
        };

        // second round: B2A
        // OT sender send
        let (y0s, us) = { bit_comp_as_ot_sender_batch::<_, A>(&inputs_0, delta, &qs[..num_bits]) };
        // OT receiver receive
        let y1s = {
            let us = serialize_and_deserialize(us);
            bit_comp_as_ot_receiver_batch(&inputs_1, &msg_to_receiver.ts[..num_bits], &us)
        };

        // y = y0 + y1
        let ys = y0s
            .iter()
            .zip(y1s.iter())
            .map(|(&y0, &y1)| y0.wrapping_add(&y1))
            .collect::<Vec<_>>();

        let inputs_in_a = inputs.iter().map(|x| x.as_uint()).collect::<Vec<A>>();
        assert_eq!(inputs_in_a.len(), ys.len());
        assert_eq!(inputs_in_a, ys);
    }

    #[test]
    fn test_b2a_end_to_end() {
        b2a_end_to_end_template::<u32, u64>();
        b2a_end_to_end_template::<u8, u32>();
        b2a_end_to_end_template::<u8, u64>();
    }
}
