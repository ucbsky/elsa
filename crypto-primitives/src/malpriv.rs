//! Support for malicious privacy by using local computation of transcripts.

use serialize::Communicate;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512};

/// Hash for multiple messages.
pub trait MessageHash {
    type Output: Communicate<Deserialized = Self::Output> + PartialEq + Eq + 'static;

    /// Absorb a message.
    fn absorb<M: Communicate>(&mut self, msg: &M);

    /// Output the hash.
    fn digest(self) -> Self::Output;
}

impl MessageHash for () {
    type Output = ();

    fn absorb<M: Communicate>(&mut self, msg: &M) {
        let _ = msg;
    }

    fn digest(self) -> Self::Output {
        ()
    }
}

macro_rules! impl_msg_hash{
    ($($ty:ty),*) => {
        $(
            impl MessageHash for $ty {
                type Output = Vec<u8>;

                fn absorb<M: Communicate>(&mut self, msg: &M) {
                    let bytes = msg.into_bytes_owned();
                    self.update(&bytes[..]);
                }

                fn digest(self) -> Self::Output {
                    let out = self.finalize();
                    out.to_vec()
                }
            }
        )*
    };
}

impl_msg_hash!(Sha224, Sha256, Sha384, Sha512);

pub mod client {
    use crate::{
        a2s::batch_a2s_first,
        b2a::{bit_comp_as_ot_receiver_batch, bit_comp_as_ot_sender_batch},
        bits::BitsLE,
        cot::{
            client::{num_additional_ot_needed, B2ACOTToAlice, B2ACOTToBob},
            server::{sample_chi, OTReceiver},
        },
        malpriv::MessageHash,
        square_corr::SquareCorrShare,
        uint::UInt,
        utils::SliceExt,
        ALICE, BOB,
    };
    use rand::{rngs::StdRng, SeedableRng};
    use serialize::AsUseCast;

    /// Simulate B2A on both sides, hashing sent message using `hasher`.
    ///
    /// # Return
    /// * `y0`: Arithmetic share of input.
    /// * `y1`: Arithmetic share of input.
    pub fn simulate_b2a<I, A, H>(
        inputs_0: &[BitsLE<I>],
        inputs_1: &[BitsLE<I>],
        cot_alice: &B2ACOTToAlice,
        cot_bob: &B2ACOTToBob,
        hasher_ab: &mut H,
    ) -> (Vec<A>, Vec<A>)
    where
        I: UInt,
        A: UInt,
        H: MessageHash,
    {
        let gsize = inputs_0.len();
        assert_eq!(inputs_1.len(), gsize);
        let num_ot = gsize * I::NUM_BITS as usize;
        let qs = cot_alice.qs_seed.expand(num_ot);
        let qs = &qs[..num_ot];
        let ts = &cot_bob.ts[..num_ot];

        let (y0, us) = bit_comp_as_ot_sender_batch::<I, A>(&inputs_0, cot_alice.delta, &qs);
        let y1 = bit_comp_as_ot_receiver_batch(&inputs_1, &ts, &us);
        hasher_ab.absorb(&us);
        (y0, y1)
    }

    /// Simulate A2S on both sides, hashing sent message using `hasher`.
    pub fn simulate_a2s<I, A, C, H>(
        gsize: usize,
        sqcorr_alice: &[SquareCorrShare<C>],
        sqcorr_bob: &[SquareCorrShare<C>],
        y0: &[A],
        y1: &[A],
        hasher_ab: &mut H,
        hasher_ba: &mut H,
    ) where
        I: UInt,
        A: UInt,
        C: UInt,
        H: MessageHash,
    {
        assert_eq!(y0.len(), gsize);
        assert_eq!(y1.len(), gsize);

        let corr0 = &sqcorr_alice[..gsize]
            .iter()
            .map(|x| x.cut())
            .collect::<Vec<_>>();
        let corr1 = &sqcorr_bob[..gsize]
            .iter()
            .map(|x| x.cut())
            .collect::<Vec<_>>();

        let e0 = batch_a2s_first(y0, &corr0);
        let e1 = batch_a2s_first(y1, &corr1);

        hasher_ab.absorb(&e0);
        hasher_ba.absorb(&e1);

        // transcript for secure comparison is ignored here
    }

    /// Simulate OT verification on both sides. (Simulation not needed for
    /// Alice)
    pub fn simulate_ot_verify<I, A, H>(
        inputs_1: &[BitsLE<I>],
        cot: &B2ACOTToBob,
        chi_seed: u64,
        hasher_ba: &mut H,
    ) where
        I: UInt,
        A: UInt,
        H: MessageHash,
    {
        let num_ot = inputs_1.len() * I::NUM_BITS as usize;
        let num_additional_ot = num_additional_ot_needed(num_ot);
        let chi = sample_chi(num_ot + num_additional_ot, chi_seed);
        let (x_til, t_til) = OTReceiver::send_x_til_t_til(&cot.ts, &chi, &inputs_1, cot.r_seed);

        hasher_ba.absorb(&(x_til.use_cast(), t_til));
    }

    /// Simulate square correlation verification on both sides.
    pub fn simulate_sqcorr_verify<I, A, C, H>(
        gsize: usize,
        sqcorr_alice: &[SquareCorrShare<C>],
        sqcorr_bob: &[SquareCorrShare<C>],
        t_seed: u64,
        hasher_ab: &mut H,
        hasher_ba: &mut H,
    ) where
        I: UInt,
        A: UInt,
        C: UInt,
        H: MessageHash,
    {
        let mut t_rng = StdRng::seed_from_u64(t_seed);

        let mut d0 = vec![C::zero(); gsize];
        let mut d1 = vec![C::zero(); gsize];

        assert_eq!(sqcorr_alice.len(), gsize * 2);
        assert_eq!(sqcorr_bob.len(), gsize * 2);

        let corr_0 = &sqcorr_alice[..gsize];
        let sacr_0 = &sqcorr_alice[gsize..];
        let corr_1 = &sqcorr_bob[..gsize];
        let sacr_1 = &sqcorr_bob[gsize..];

        let t = (0..gsize).map(|_| C::rand(&mut t_rng)).collect::<Vec<_>>();

        SquareCorrShare::verify_phase_1(corr_0, sacr_0, &t, &mut d0);
        SquareCorrShare::verify_phase_1(corr_1, sacr_1, &t, &mut d1);

        // println!("d0: {:x?}, d1: {:x?}", d0, d1);

        hasher_ab.absorb(&d0);
        hasher_ba.absorb(&d1);

        let d = d0.zip_map(&d1, |x, y| x.wrapping_add(y));

        let mut w0 = vec![C::zero(); gsize];
        let mut w1 = vec![C::zero(); gsize];

        SquareCorrShare::verify_phase_2::<{ ALICE }>(corr_0, sacr_0, &t, &d, &mut w0);
        SquareCorrShare::verify_phase_2::<{ BOB }>(corr_1, sacr_1, &t, &d, &mut w1);

        hasher_ab.absorb(&w0);
        hasher_ba.absorb(&w1); // TODO change back
    }
}
