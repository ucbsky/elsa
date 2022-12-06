//! Defines Naive ROT.

use crate::{
    bits::PackedBits,
    cot::{
        rot::{cot_to_rot_receiver_side, cot_to_rot_sender_side},
        server::{inner_product, inner_product_with_boolean_scalar, OTSender},
        COTSeed, ChoiceSeed,
    },
};
use block::{gf::GF2_256, Block};
use std::ops::Range;

pub struct NaiveCOTsForSender {
    pub delta: Block,
    pub qs_seed: COTSeed,
}

pub struct NaiveROTsForSender {
    pub v0: Vec<bool>,
    pub v1: Vec<bool>,
}

impl NaiveCOTsForSender {
    /// Return `qs`, the first element of each COT, and whether verification
    /// succeeded.
    pub fn verify_and_get_cots(
        &self,
        chi: &[Block],
        x_til: Block,
        t_til: GF2_256,
    ) -> (Vec<Block>, bool) {
        OTSender::verify_and_get_cot(self.qs_seed, chi, self.delta, x_til, t_til)
    }

    pub fn to_rot(&self, qs: &[Block]) -> NaiveROTsForSender {

        let raw = cot_to_rot_sender_side::<u8>(qs, self.delta);
        let v0 = raw.0.into_iter().map(|x| x & 1 == 1).collect();
        let v1 = raw.1.into_iter().map(|x| x & 1 == 1).collect();
        NaiveROTsForSender { v0, v1 }
    }
}

impl NaiveROTsForSender {
    /// return `v0[range]`, `v1[range]`
    pub fn get_range(&self, range: Range<usize>) -> (PackedBits, PackedBits) {
        let v0 = self.v0[range.clone()]
            .iter()
            .copied()
            .collect::<PackedBits>();
        let v1 = self.v1[range].iter().copied().collect::<PackedBits>();
        (v0, v1)
    }
}

pub struct NaiveCOTsForReceiver {
    pub ts: Vec<Block>,
    /// Choice seed expands a vector of random booleans of size `num_cots +
    /// num_additional_cots_for_verify`
    pub choice_seed: ChoiceSeed,
}

pub struct NaiveROTsForReceiver {
    /// selected rots
    pub v: Vec<bool>,
    /// the select bits
    pub vb: Vec<bool>,
}

impl NaiveCOTsForReceiver {
    /// send `x_til` and `t_til` to the sender for verification
    pub fn send_x_til_and_t_til(&self, chi: &[Block]) -> (Block, GF2_256) {
        assert_eq!(chi.len(), self.ts.len());
        // generate x_hat
        let x_hat = self.choice_seed.expand(chi.len());
        debug_assert_eq!(x_hat.len(), chi.len());
        let x_til = inner_product_with_boolean_scalar(x_hat.iter(), chi);
        let t_til = inner_product(&self.ts, chi);

        (x_til, t_til)
    }

    pub fn to_rot(&self, num_rots: usize) -> NaiveROTsForReceiver {
        let raw = cot_to_rot_receiver_side::<u8>(&self.ts[..num_rots]);
        let v = raw.into_iter().map(|x| x & 1 == 1).collect();
        let vb = self.choice_seed.expand(num_rots).iter().collect();
        NaiveROTsForReceiver { v, vb }
    }
}

impl NaiveROTsForReceiver {
    /// return `v[range]`, `vb[range]`
    pub fn get_range(&self, range: Range<usize>) -> (PackedBits, PackedBits) {
        let v = self.v[range.clone()]
            .iter()
            .copied()
            .collect::<PackedBits>();
        let vb = self.vb[range].iter().copied().collect::<PackedBits>();
        (v, vb)
    }
}

pub struct NaiveCOTAlice {
    pub straight: NaiveCOTsForSender,
    pub reverse: NaiveCOTsForReceiver,
}

impl NaiveCOTAlice {
    /// generate x_til and t_til for reverse pool
    pub fn generate_verify_message(&self, chi: &[Block]) -> (Block, GF2_256) {
        self.reverse.send_x_til_and_t_til(chi)
    }

    pub fn verify_and_get_qs_straight(
        &self,
        chi: &[Block],
        x_til: Block,
        t_til: GF2_256,
    ) -> (Vec<Block>, bool) {
        self.straight.verify_and_get_cots(chi, x_til, t_til)
    }

    pub fn to_rot(&self, num_rots: usize, qs_straight: &[Block]) -> NaiveROTAlice {
        let straight = self.straight.to_rot(&qs_straight[..num_rots]);
        let reverse = self.reverse.to_rot(num_rots);
        NaiveROTAlice { straight, reverse }
    }
}

pub struct NaiveROTAlice {
    pub straight: NaiveROTsForSender,
    pub reverse: NaiveROTsForReceiver,
}

impl NaiveROTAlice {
    /// return `v0[range]`, `v1[range]` from straight pool, and `w[range]`, `wb[range]` from reverse pool
    pub fn get_range(&self, range: Range<usize>) -> ((PackedBits, PackedBits), (PackedBits, PackedBits)) {
        (self.straight.get_range(range.clone()), self.reverse.get_range(range))
    }
}


pub struct NaiveCOTBob {
    pub straight: NaiveCOTsForReceiver,
    pub reverse: NaiveCOTsForSender,
}

impl NaiveCOTBob {
    /// generate x_til and t_til for straight pool
    pub fn generate_verify_message(&self, chi: &[Block]) -> (Block, GF2_256) {
        self.straight.send_x_til_and_t_til(chi)
    }

    pub fn verify_and_get_qs_reverse(
        &self,
        chi: &[Block],
        x_til: Block,
        t_til: GF2_256,
    ) -> (Vec<Block>, bool) {
        self.reverse.verify_and_get_cots(chi, x_til, t_til)
    }

    pub fn to_rot(&self, num_rots: usize, qs_reverse: &[Block]) -> NaiveROTBob {
        let reverse = self.reverse.to_rot(&qs_reverse[..num_rots]);
        let straight = self.straight.to_rot(num_rots);
        NaiveROTBob { reverse, straight }
    }
}

pub struct NaiveROTBob {
    pub straight: NaiveROTsForReceiver,
    pub reverse: NaiveROTsForSender,
}

impl NaiveROTBob {
    /// return `v[range]`, `vb[range]` from straight pool, and `w0[range]`, `w1[range]` from reverse pool
    pub fn get_range(&self, range: Range<usize>) -> ((PackedBits, PackedBits), (PackedBits, PackedBits)) {
        (self.straight.get_range(range.clone()), self.reverse.get_range(range))
    }
}


pub mod clients {
    use crate::cot::{
        client::num_additional_ot_needed,
        naive_rot::{NaiveCOTAlice, NaiveCOTBob, NaiveCOTsForReceiver, NaiveCOTsForSender},
        COTSeed, ChoiceSeed,
    };
    use block::Block;
    use rand::Rng;

    fn generate_cots_assist<R: Rng>(
        rng: &mut R,
        num_cots_in_each_pool: usize,
    ) -> (NaiveCOTsForSender, NaiveCOTsForReceiver) {
        let additional = num_additional_ot_needed(num_cots_in_each_pool);
        // generate straight pool
        let delta = Block::rand(rng);
        let qs_seed = COTSeed(Block::rand(rng));
        let choice_seed = ChoiceSeed(rng.gen());
        let choices = choice_seed.expand(num_cots_in_each_pool + additional);
        let ts = qs_seed.expand_selected(num_cots_in_each_pool + additional, delta, choices.iter());
        let send = NaiveCOTsForSender { delta, qs_seed };
        let recv = NaiveCOTsForReceiver { ts, choice_seed };
        (send, recv)
    }

    pub fn generate_naive_cots<R: Rng>(
        rng: &mut R,
        num_cots_in_each_pool: usize,
    ) -> (NaiveCOTAlice, NaiveCOTBob) {
        let (straight_send, straight_recv) = generate_cots_assist(rng, num_cots_in_each_pool);
        let (reverse_send, reverse_recv) = generate_cots_assist(rng, num_cots_in_each_pool);
        (
            NaiveCOTAlice {
                straight: straight_send,
                reverse: reverse_recv,
            },
            NaiveCOTBob {
                straight: straight_recv,
                reverse: reverse_send,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::cot::naive_rot::clients::generate_naive_cots;
    use block::Block;
    use rand::{rngs::StdRng, SeedableRng};
    use crate::cot::client::num_additional_ot_needed;
    use crate::cot::naive_rot::{NaiveCOTsForReceiver, NaiveCOTsForSender};

    fn check_naive_cot_consistency(send: &NaiveCOTsForSender, recv: &NaiveCOTsForReceiver){
        let num_cots = recv.ts.len();
        let choices = recv.choice_seed.expand(num_cots);
        let expected = send.qs_seed.expand_selected(num_cots, send.delta, choices.iter());
        let actual = recv.ts.clone();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_naive_rot() {
        const SIZE: usize = 1000;
        let mut rng = StdRng::seed_from_u64(12345);
        let (cot_alice, cot_bob) = generate_naive_cots(&mut rng, SIZE);

        check_naive_cot_consistency(&cot_alice.straight, &cot_bob.straight);
        check_naive_cot_consistency(&cot_bob.reverse, &cot_alice.reverse);

        // verify
        let chi = (0..(SIZE + num_additional_ot_needed(SIZE))).map(|_| Block::rand(&mut rng)).collect::<Vec<_>>();
        let msg_alice = cot_alice.generate_verify_message(&chi);
        let msg_bob = cot_bob.generate_verify_message(&chi);
        let (qs_straight_alice, verify_result_alice) =
            cot_alice.verify_and_get_qs_straight(&chi, msg_bob.0, msg_bob.1);
        let (qs_reverse_bob, verify_result_bob) =
            cot_bob.verify_and_get_qs_reverse(&chi, msg_alice.0, msg_alice.1);

        assert!(verify_result_alice);
        assert!(verify_result_bob);

        let rot_alice = cot_alice.to_rot(SIZE, &qs_straight_alice);
        let rot_bob = cot_bob.to_rot(SIZE, &qs_reverse_bob);

        assert_eq!(rot_alice.straight.v0.len(), SIZE);
        assert_eq!(rot_alice.straight.v1.len(), SIZE);
        assert_eq!(rot_alice.reverse.v.len(), SIZE);
        assert_eq!(rot_alice.reverse.vb.len(), SIZE);

        assert_eq!(rot_bob.straight.v.len(), SIZE);
        assert_eq!(rot_bob.straight.vb.len(), SIZE);
        assert_eq!(rot_bob.reverse.v0.len(), SIZE);
        assert_eq!(rot_bob.reverse.v1.len(), SIZE);


        // straight pool
        for i in 0..SIZE {
            let bob_val = rot_bob.straight.v[i];
            let alice_val = if rot_bob.straight.vb[i] {
                rot_alice.straight.v1[i]
            } else {
                rot_alice.straight.v0[i]
            };
            assert_eq!(bob_val, alice_val, "at: {}", i);
        }

        // reverse pool
        for i in 0..SIZE {
            let alice_val = rot_alice.reverse.v[i];
            let bob_val = if rot_alice.reverse.vb[i] {
                rot_bob.reverse.v1[i]
            } else {
                rot_bob.reverse.v0[i]
            };
            assert_eq!(alice_val, bob_val, "at: {}", i);
        }
    }
}
