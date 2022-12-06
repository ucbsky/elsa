//! Adapted from https://github.com/emp-toolkit/emp-tool/blob/b07a7d9ab3/emp-tool/utils/aes_opt.h
//! Reference: Implementation of "Fast Garbling of Circuits Under Standard
//! Assumptions" https://eprint.iacr.org/2015/751.pdf

use safe_arch::{
    aes_encrypt_last_m128i, aes_encrypt_m128i, bitxor_m128i, m128i, shl_imm_u32_m128i,
    shl_imm_u64_m128i, shuffle_av_i8z_all_m128i,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct AESKey {
    pub rd_key: [m128i; 11],
    pub rounds: u32,
}

fn ks_rounds(keys_dest: &mut [AESKey], con: m128i, con3: m128i, mask: m128i, r: usize) {
    keys_dest.iter_mut().for_each(|k| {
        let mut key = k.rd_key[r - 1];
        let x2 = shuffle_av_i8z_all_m128i(key, mask);
        let aux = aes_encrypt_last_m128i(x2, con);

        let mut glob_aux = shl_imm_u64_m128i::<32>(key);
        key = bitxor_m128i(glob_aux, key);
        glob_aux = shuffle_av_i8z_all_m128i(key, con3);
        key = bitxor_m128i(glob_aux, key);
        k.rd_key[r] = bitxor_m128i(aux, key);
    })
}

// AES key scheduling for 8 keys
// [REF] Implementation of "Fast Garbling of Circuits Under Standard
// Assumptions" https://eprint.iacr.org/2015/751.pdf
pub fn aes_opt_key_schedule<const NUM_KEYS: usize>(
    user_key: &[m128i; NUM_KEYS],
    keys_dest: &mut [AESKey; NUM_KEYS],
) {
    assert_eq!(user_key.len(), keys_dest.len());

    let mut con = m128i::from([1u32, 1, 1, 1]);
    let mut con2 = m128i::from([0x1bu32, 0x1b, 0x1b, 0x1b]);
    let con3 = m128i::from([0x0ffffffffu32, 0x0ffffffffu32, 0x07060504, 0x07060504]);
    let mask = m128i::from([0x0c0f0e0du32, 0x0c0f0e0du32, 0x0c0f0e0du32, 0x0c0f0e0du32]);

    keys_dest
        .iter_mut()
        .zip(user_key.iter())
        .for_each(|(k, uk)| {
            k.rd_key[0] = *uk;
            k.rounds = 10;
        });

    ks_rounds(keys_dest, con, con3, mask, 1);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 2);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 3);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 4);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 5);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 6);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 7);
    con = shl_imm_u32_m128i::<1>(con);
    ks_rounds(keys_dest, con, con3, mask, 8);

    ks_rounds(keys_dest, con2, con3, mask, 9);
    con2 = shl_imm_u32_m128i::<1>(con2);
    ks_rounds(keys_dest, con2, con3, mask, 10);
}

/// With `keys.len()` keys, use each key to encrypt `NUM_ENCS` blocks.
///
/// The key applied on
/// (`blocks[0]`, `blocks[1]`, ... , `blocks[NUM_ENCS-1]`) are the same, and so on.
/// # Panics (debug only)
///
/// Panics if `blocks.len() != NUM_ENCS * NUM_KEYS`. .
// Adapted from https://github.com/emp-toolkit/emp-tool/blob/b07a7d9ab3053a3e16991751402742d418377f63/emp-tool/utils/aes_opt.h#L64
pub(crate) fn para_enc<const NUM_ENCS: usize, const NUM_KEYS: usize, const INPUT_SIZE: usize>(
    blocks: &mut [m128i; INPUT_SIZE],
    keys: &[AESKey; NUM_KEYS],
) {
    debug_assert_eq!(blocks.len(), NUM_ENCS * NUM_KEYS);

    // first, xor key
    blocks
        .chunks_mut(NUM_ENCS)
        .zip(keys.iter().map(|k| k.rd_key[0]))
        .for_each(|(bs, k)| {
            bs.iter_mut().for_each(|b| {
                *b = *b ^ k;
            })
        });

    // for each round, do AES encrypt
    for r in 1..10 {
        blocks
            .chunks_mut(NUM_ENCS)
            .zip(keys.iter().map(|k| k.rd_key[r]))
            .for_each(|(bs, k)| bs.iter_mut().for_each(|b| *b = aes_encrypt_m128i(*b, k)))
    }

    // last round encryption
    blocks
        .chunks_mut(NUM_ENCS)
        .zip(keys.iter().map(|k| k.rd_key[10]))
        .for_each(|(bs, k)| {
            bs.iter_mut()
                .for_each(|b| *b = aes_encrypt_last_m128i(*b, k))
        })
}

pub fn aes_ecb_encrypt_blocks(blocks: &mut [m128i], key: &AESKey) {
    blocks.iter_mut().for_each(|b|*b = *b ^ key.rd_key[0]);
    for j in 1..key.rounds {
        blocks.iter_mut().for_each(|b| *b = aes_encrypt_m128i(*b, key.rd_key[j as usize]))
    }
    blocks.iter_mut().for_each(|b|*b = aes_encrypt_last_m128i(*b, key.rd_key[key.rounds as usize]))
}