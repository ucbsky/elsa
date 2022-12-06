//! Client side algorithms for generating ROT.

use crate::{bits::BitsLE, uint::UInt};
use block::Block;
use rand::Rng;
use serialize::{AsUseCast, Communicate, UseCast};
use std::{
    io::{Read, Write},
    mem::size_of,
};

use super::{COTSeed, ChoiceSeed};

/// Generate ROT.
pub struct COTGen {}

/// Given `gsize` and `wsize`, we need `gsize * wsize` OTs and some additional
/// OTs for verification. This function returns the number of additional OTs
/// needed for verification.
/// `num_ot_used`: `gsize * wsize`
pub fn num_additional_ot_needed(_num_ot_used: usize) -> usize {
    194
}

#[derive(Clone, Debug, Default)]
/// For B2A, Alice is always the OT sender.
pub struct B2ACOTToAlice {
    pub delta: Block,
    pub qs_seed: COTSeed,
}

impl Communicate for B2ACOTToAlice {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        size_of::<Block>() + size_of::<COTSeed>()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        self.delta.use_cast().to_bytes(&mut dest);
        self.qs_seed.use_cast().to_bytes(&mut dest);
    }

    fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
        let delta = UseCast::<Block>::from_bytes(&mut bytes)?;
        let qs_seed = UseCast::<COTSeed>::from_bytes(&mut bytes)?;
        Ok(B2ACOTToAlice { delta, qs_seed })
    }
}

impl B2ACOTToAlice {
    pub fn new(delta: Block, qs_seed: COTSeed) -> Self {
        B2ACOTToAlice { delta, qs_seed }
    }
}

#[derive(Clone, Debug, Default)]
/// For B2A, Bob is always the OT receiver.
pub struct B2ACOTToBob {
    pub r_seed: ChoiceSeed,
    pub ts: Vec<Block>,
}

impl Communicate for B2ACOTToBob {
    type Deserialized = Self;

    fn size_in_bytes(&self) -> usize {
        size_of::<ChoiceSeed>() + self.ts.len() * size_of::<Block>()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        self.r_seed.use_cast().to_bytes(&mut dest);
        self.ts.to_bytes(&mut dest);
    }

    fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
        let r_seed = UseCast::<ChoiceSeed>::from_bytes(&mut bytes)?;
        let ts = <Vec<Block>>::from_bytes(&mut bytes)?;
        Ok(B2ACOTToBob { r_seed, ts })
    }
}

impl B2ACOTToBob {
    pub fn new(r_seed: ChoiceSeed, ts: Vec<Block>) -> Self {
        B2ACOTToBob { r_seed, ts }
    }
}

impl COTGen {
    /// Sample Delta
    pub fn sample_delta<R: Rng>(rng: &mut R) -> Block {
        Block::rand(rng)
    }

    /// Generate `size` correlated OTs.
    /// * `rng`: RNG
    /// * `inputs_1`: inputs bits for OT receiver party
    /// * `delta`: COT delta
    /// * `num_additional`: number of additional OTs needed for verification
    ///
    /// ## Returns
    ///
    /// * `ClientCOTMsgToSender`: seed to generate `Q||Q'`, delta
    /// * `ClientCOTMsgToReceiver`: seed to generate `r
    ///
    /// * `Q`: sampled uniformly at random.
    /// * `T = Q + input_1 * delta` where choice is in `{0, 1}`, in `choices`.
    /// In addition, this function will sample `Q'` and `T'` where choice is
    /// random.
    /// * `Q'`: sampled uniformly at random.
    /// * `T' = Q' + choice * delta` where choice is in `{0, 1}`, in `r`
    ///   (randomly generated).
    ///
    /// This function will return a seed to generate `Q||Q'` and delta for OT
    /// sender; a seed to generate `r`, and `T||T'` for OT receiver.
    pub fn sample_cots<R: Rng, T: UInt>(
        rng: &mut R,
        inputs_1: &[BitsLE<T>],
        delta: Block,
        num_additional: usize,
    ) -> (B2ACOTToAlice, B2ACOTToBob) {
        let cot_rng_seed = COTSeed(Block::rand(rng));
        let choice_rng_seed = ChoiceSeed(rng.next_u64());

        let choices = inputs_1.iter().flat_map(|x| x.iter());

        let r = choice_rng_seed.expand(num_additional);

        let choices = choices.chain(r.iter());

        let ts = cot_rng_seed.expand_selected(
            inputs_1.len() * T::NUM_BITS + num_additional,
            delta,
            choices,
        );

        (
            B2ACOTToAlice::new(delta, cot_rng_seed),
            B2ACOTToBob::new(choice_rng_seed, ts),
        )
    }

    /// Generate `size` correlated OTs.
    /// * `rng`: RNG
    /// * `selected_bits`: bits that should be selected by the OT receiver
    /// * `delta`: COT delta
    /// * `num_additional`: number of additional OTs needed for verification
    ///
    /// ## Returns
    ///
    /// * `ClientCOTMsgToSender`: seed to generate `Q||Q'`, delta
    /// * `ClientCOTMsgToReceiver`: seed to generate `r
    ///
    /// * `Q`: sampled uniformly at random.
    /// * `T = Q + input_1 * delta` where choice is in `{0, 1}`, in `choices`.
    /// In addition, this function will sample `Q'` and `T'` where choice is
    /// random.
    /// * `Q'`: sampled uniformly at random.
    /// * `T' = Q' + choice * delta` where choice is in `{0, 1}`, in `r`
    ///   (randomly generated).
    ///
    /// This function will return a seed to generate `Q||Q'` and delta for OT
    /// sender; a seed to generate `r`, and `T||T'` for OT receiver.
    pub fn sample_cots_using_selected_bits<R: Rng>(
        rng: &mut R,
        choice_bits: impl IntoIterator<Item = bool>,
        num_choice_bits: usize,
        delta: Block,
        num_additional: usize,
    ) -> (B2ACOTToAlice, B2ACOTToBob) {
        let cot_rng_seed = COTSeed(Block::rand(rng));
        let choice_rng_seed = ChoiceSeed(rng.next_u64());

        let qs = cot_rng_seed.expand(num_choice_bits + num_additional);

        let r = choice_rng_seed.expand(num_additional);

        let choices = choice_bits.into_iter().chain(r.iter());

        let ts = qs
            .into_iter()
            .zip(choices)
            .map(|(q, choice)| if choice { q.add_gf(delta) } else { q })
            .collect();

        (
            B2ACOTToAlice::new(delta, cot_rng_seed),
            B2ACOTToBob::new(choice_rng_seed, ts),
        )
    }
}
