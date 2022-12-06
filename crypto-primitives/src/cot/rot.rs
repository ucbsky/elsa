//! Suppose we have COT as `q` and `t = q + select_bit * delta`. This module
//! provides function to convert `COT` to trimmed `ROT`.
use crate::{block_crypto::mitccrh::MiTCCR, uint::UInt};
use block::Block;
use bytemuck::Zeroable;
use safe_arch::m128i;

/// Start point for MitCCR Hash. This start point is arbitrary. Just make sure
/// it's consistent.
const START_POINT: [u32; 4] = [0x1234, 0x2345, 0x3456, 0x4567];
/// Batch size for COT to ROT conversion.
const OT_BSIZE: usize = 8;

/// Suppose I'm OT sender and I have vector `q`. This function calculates ROT of
/// `q` and `q + delta` and trim them to ring size.
pub fn cot_to_rot_sender_side<T: UInt>(q: &[Block], delta: Block) -> (Vec<T>, Vec<T>) {
    // in our application, `q` is always aligned to `OT_BSIZE` because `T::NUM_BITS % OT_BSIZE == 0`
    // if assertion failed, that means we probably included extra OT here
    assert_eq!(q.len() % OT_BSIZE, 0, "q is not aligned to OT_BSIZE");

    let mut crh = MiTCCR::<OT_BSIZE>::new(START_POINT.into());

    const PAD_SIZE: usize = OT_BSIZE * 2;
    let mut pad = [m128i::zeroed(); PAD_SIZE];
    let mut data_0 = Vec::<T>::with_capacity(q.len());
    let mut data_1 = Vec::<T>::with_capacity(q.len());

    q.chunks_exact(OT_BSIZE).for_each(|qs| {
        // each qs is of size OT_BSIZE, let's cast it to array
        qs.iter().zip(pad.chunks_mut(2)).for_each(|(q, p)| {
            p[0] = q.0;
            p[1] = q.add_gf(delta).0;
        });
        crh.hash::<2, PAD_SIZE>(&mut pad);
        // we take `qs.len()` to address padding
        pad.chunks_mut(2).for_each(|p| {
            data_0.push(T::from_rot(p[0]));
            data_1.push(T::from_rot(p[1]));
        });
    });

    (data_0, data_1)
}

/// Suppose I'm OT receiver and I have vector `t = q + select_bit * delta`. This function
/// calculates ROT of `t` and trim it to ring size.
pub fn cot_to_rot_receiver_side<T: UInt>(t: &[Block]) -> Vec<T> {
    // in our application, `t` is always aligned to `OT_BSIZE` because `T::NUM_BITS % OT_BSIZE == 0`
    // if assertion failed, that means we probably included extra OT here
    assert_eq!(t.len() % OT_BSIZE, 0, "t is not aligned to OT_BSIZE");

    let mut crh = MiTCCR::<OT_BSIZE>::new(START_POINT.into());
    const PAD_SIZE: usize = OT_BSIZE;
    let mut pad = [m128i::zeroed(); PAD_SIZE];
    let mut data = Vec::<T>::with_capacity(t.len());

    t.chunks(OT_BSIZE).for_each(|qs|{
        pad.copy_from_slice(bytemuck::cast_slice(qs));
        crh.hash::<1, PAD_SIZE>(&mut pad);
        data.extend(pad.iter().map(|p| T::from_rot(*p)));
    });

    data
}