//! A random number generator specialized for Block.

use crate::block_crypto::aes::{aes_ecb_encrypt_blocks, aes_opt_key_schedule, AESKey};
use block::Block;
use rand::random;
use safe_arch::m128i;

pub struct BlockRng {
    counter: u64,
    aes: AESKey,
}

impl BlockRng {
    pub fn new(seed: Option<Block>) -> Self {
        let seed = match seed {
            Some(seed) => seed.0,
            None => {
                let r0: u64 = random();
                let r1 = random();
                m128i::from([r0, r1])
            },
        };

        let mut aes = [AESKey::default()];
        aes_opt_key_schedule(&[seed], &mut aes);
        let counter = 0;
        Self {
            counter,
            aes: aes[0],
        }
    }

    pub fn random_blocks(&mut self, blocks_dest: &mut [Block]) {
        const AES_BATCH_SIZE: usize = 8;
        let blocks_dest = bytemuck::cast_slice_mut::<_, m128i>(blocks_dest);
        (0..blocks_dest.len() / AES_BATCH_SIZE).for_each(|i| {
            let window = &mut blocks_dest[i * AES_BATCH_SIZE..(i + 1) * AES_BATCH_SIZE];
            window
                .iter_mut()
                .zip(0..AES_BATCH_SIZE as u64)
                .for_each(|(dest, _)| {
                    *dest = m128i::from([self.counter, 0]);
                    self.counter += 1;
                });
            aes_ecb_encrypt_blocks(window, &self.aes);
        });
        let remain = blocks_dest.len() % AES_BATCH_SIZE;
        let r = blocks_dest.len() - remain;
        let window = &mut blocks_dest[r..];
        (0..remain).for_each(|j| {
            window[j] = m128i::from([self.counter, 0]);
            self.counter += 1;
        });
        aes_ecb_encrypt_blocks(&mut window[..remain], &self.aes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanity() {
        let seed = Block(0x1234567890abcdefu128.into());
        let mut rng1 = BlockRng::new(Some(seed));
        let mut data1 = [Block::default(); 7];
        let mut data2 = [Block::default(); 19];
        let mut data3 = [Block::default(); 64];

        rng1.random_blocks(&mut data1);
        rng1.random_blocks(&mut data2);
        rng1.random_blocks(&mut data3);

        assert_ne!(data1[..4], data2[..4]);

        let mut rng2 = BlockRng::new(Some(seed));
        let mut data4 = [Block::default(); 7];
        let mut data5 = [Block::default(); 19];
        let mut data6 = [Block::default(); 64];

        rng2.random_blocks(&mut data4);
        rng2.random_blocks(&mut data5);
        rng2.random_blocks(&mut data6);

        assert_eq!(data1[..4], data4[..4]);
        assert_eq!(data2[..4], data5[..4]);
        assert_eq!(data3[..4], data6[..4]);
    }
}
