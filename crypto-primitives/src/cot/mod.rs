//! This module contains code for generating and verifying random COT.
//! There are two server parties (Alice and Bob) and one client.
//! For load balancing,

use crate::{bits::PackedBits, block_crypto::rng::BlockRng};
use block::Block;
use bytemuck::{Pod, Zeroable};
use rand::{rngs::StdRng, SeedableRng};
use serialize::{AsUseCast, Communicate, UseCast};
use std::io::{Read, Write};

pub mod client;
pub mod naive_rot;
pub mod rot;
pub mod server;

/// A seed to randomly generate COT deterministically.
#[derive(Debug, Clone, Copy, Pod, Zeroable, Default)]
#[repr(transparent)]
pub struct COTSeed(pub Block);

impl COTSeed {
    #[allow(clippy::uninit_vec)]
    pub fn expand(&self, num_cots: usize) -> Vec<Block> {
        let mut cot_rng = BlockRng::new(Some(self.0));
        // safety: `Block` is a primitive type, and has no destructors
        let mut qs = Vec::with_capacity(num_cots);
        unsafe {
            qs.set_len(num_cots);
        }
        cot_rng.random_blocks(&mut qs);
        qs
    }

    pub fn expand_selected(
        &self,
        num_cots: usize,
        delta: Block,
        select: impl IntoIterator<Item = bool>,
    ) -> Vec<Block> {
        let qs = self.expand(num_cots);
        qs.into_iter()
            .zip(select)
            .map(|(q, choice)| if choice { q.add_gf(delta) } else { q })
            .collect()
    }
}

impl Communicate for COTSeed {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        self.0.use_cast().size_in_bytes()
    }

    fn to_bytes<W: Write>(&self, dest: W) {
        self.0.use_cast().to_bytes(dest);
    }

    fn from_bytes<R: Read>(bytes: R) -> serialize::Result<Self::Deserialized> {
        Ok(COTSeed(UseCast::<Block>::from_bytes(bytes)?))
    }
}

/// A seed to randomly generate choices (`r`) deterministically.
#[derive(Debug, Clone, Copy, Pod, Zeroable, Default)]
#[repr(transparent)]
pub struct ChoiceSeed(pub u64);

impl ChoiceSeed {
    pub fn expand(&self, r_size: usize) -> PackedBits {
        let mut choice_rng = StdRng::seed_from_u64(self.0);
        let r = PackedBits::rand(&mut choice_rng, r_size);
        r
    }
}
