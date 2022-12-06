use crypto_primitives::{
    bits::batch_make_boolean_shares,
    cot::client::{num_additional_ot_needed, COTGen},
    malpriv::{
        client::{simulate_a2s, simulate_b2a, simulate_ot_verify, simulate_sqcorr_verify},
        MessageHash,
    },
    message::l2::{ClientL2MsgToAlice, ClientL2MsgToBob, ClientMPMsgToAlice, ClientMPMsgToBob},
    square_corr::batch_make_sqcorr_shares,
    uint::UInt,
    utils::bytes_to_seed_pairs,
};
use rand::Rng;

/// Client on input ring `I`, and correlation ring `C`
pub struct Client<I: UInt, C: UInt, H: MessageHash> {
    pub msg_alice: ClientMPMsgToAlice<H>, // phase 1 and phase 2
    pub msg_bob: ClientMPMsgToBob<I, C, H>,
}

impl<I: UInt, C: UInt, H: MessageHash<Output = Vec<u8>>> Client<I, C, H> {
    /// use Fiat-Shamir to combine two messages
    pub fn prepare_message<A: UInt, R: Rng, F>(input: &[I], rng: &mut R, hasher: F) -> Self
    where
        F: Fn() -> H,
    {
        let mut hasher_b2a_ab = hasher(); // hasher of message sent from alice to bob
        let mut hasher_a2s_ab = hasher(); // hasher of message sent from alice to bob
        let mut hasher_a2s_ba = hasher(); // hasher of message sent from bob to alice

        let gsize = input.len();
        let (inputs_0, inputs_1) =
            batch_make_boolean_shares(rng, input.iter().map(|x| x.bits_le()));
        let inputs_0_expanded = inputs_0.expand(gsize);
        let delta = COTGen::sample_delta(rng);
        let num_additional_cot = num_additional_ot_needed(gsize * I::NUM_BITS as usize);
        let (cot_s, cot_r) = COTGen::sample_cots(rng, &inputs_1, delta, num_additional_cot);

        // generate correlation
        let (corr0, corr1, sqcorr_a, sqcorr_b) = batch_make_sqcorr_shares(rng, gsize * 2);

        let msg_alice = ClientL2MsgToAlice::new(inputs_0, cot_s, corr0);
        let msg_bob = ClientL2MsgToBob::new(inputs_1, cot_r, corr1);

        // simulate B2A and A2S and get transcript
        let (y0, y1) = simulate_b2a::<I, A, H>(
            &inputs_0_expanded,
            &msg_bob.po2_msg.inputs_1,
            msg_alice.cot(),
            msg_bob.cot(),
            &mut hasher_b2a_ab,
        );
        simulate_a2s::<I, A, C, _>(
            gsize,
            &sqcorr_a,
            &sqcorr_b,
            &y0,
            &y1,
            &mut hasher_a2s_ab,
            &mut hasher_a2s_ba,
        );

        let msg_phase1_a = (msg_alice, hasher_a2s_ba.digest());
        let msg_phase1_b = (msg_bob, hasher_b2a_ab.digest(), hasher_a2s_ab.digest());

        let mut fs_hasher_a = hasher();
        let mut fs_hasher_b = hasher();
        fs_hasher_a.absorb(&msg_phase1_a);
        fs_hasher_b.absorb(&msg_phase1_b);

        let fs_hash_a = fs_hasher_a.digest();
        let fs_hash_b = fs_hasher_b.digest();

        let (chi_seed_a, t_seed_a) = bytes_to_seed_pairs(&fs_hash_a);
        let (chi_seed_b, t_seed_b) = bytes_to_seed_pairs(&fs_hash_b);

        // XXX: ideally, we should hash the two and get a new seed here, but for now we just use XOR for simplicity
        let chi_seed = chi_seed_a ^ chi_seed_b;
        let t_seed = t_seed_a ^ t_seed_b;

        // Phase 2
        let mut hasher_ot_ba = hasher();
        let mut hasher_sqcorr_ab = hasher();
        let mut hasher_sqcorr_ba = hasher();

        // verification
        simulate_ot_verify::<I, A, H>(
            &msg_phase1_b.0.po2_msg.inputs_1,
            &msg_phase1_b.0.cot(),
            chi_seed,
            &mut hasher_ot_ba,
        );
        simulate_sqcorr_verify::<I, A, C, H>(
            gsize,
            &sqcorr_a,
            &sqcorr_b,
            t_seed,
            &mut hasher_sqcorr_ab,
            &mut hasher_sqcorr_ba,
        );

        let msg_phase2_a = (hasher_ot_ba.digest(), hasher_sqcorr_ba.digest());
        let msg_phase2_b = hasher_sqcorr_ab.digest();

        Self {
            msg_alice: (msg_phase1_a, msg_phase2_a),
            msg_bob: (msg_phase1_b, msg_phase2_b),
        }
    }
    // no need to receive from bob
}
