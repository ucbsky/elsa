use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use crypto_primitives::{
    bits::{batch_make_boolean_shares, BitsLE},
    cot::client::{num_additional_ot_needed, B2ACOTToAlice, B2ACOTToBob, COTGen},
    malpriv::client::{simulate_a2s, simulate_b2a, simulate_ot_verify, simulate_sqcorr_verify},
    square_corr::{batch_make_sqcorr_shares, SquareCorrShare},
    uint::UInt,
};
use rand::{rngs::StdRng, SeedableRng};
use sha2::Sha256;

type Hasher = Sha256;
fn msg_prepare<I: UInt, A: UInt, C: UInt>(
    input: &[I],
) -> (
    Vec<BitsLE<I>>,
    Vec<BitsLE<I>>,
    B2ACOTToAlice,
    B2ACOTToBob,
    Vec<SquareCorrShare<C>>,
    Vec<SquareCorrShare<C>>,
) {
    let mut rng = StdRng::from_entropy();
    let gsize = input.len();
    let (inputs_0, inputs_1) =
        batch_make_boolean_shares(&mut rng, input.iter().map(|x| x.bits_le()));
    let inputs_0_expanded = inputs_0.expand(gsize);
    let delta = COTGen::sample_delta(&mut rng);
    let num_additional_cot = num_additional_ot_needed(gsize * I::NUM_BITS as usize);
    let (cot_s, cot_r) = COTGen::sample_cots(&mut rng, &inputs_1, delta, num_additional_cot);

    // generate correlation
    let (_, _, corr0_expanded, corr1_expanded) = batch_make_sqcorr_shares(&mut rng, gsize * 2);

    (
        inputs_0_expanded,
        inputs_1,
        cot_s,
        cot_r,
        corr0_expanded,
        corr1_expanded,
    )
}

fn transcript_emulation<I: UInt, A: UInt, C: UInt>(
    inputs_0: &[BitsLE<I>],
    inputs_1: &[BitsLE<I>],
    cot_alice: &B2ACOTToAlice,
    cot_bob: &B2ACOTToBob,
    sqcorr_alice: &[SquareCorrShare<C>],
    sqcorr_bob: &[SquareCorrShare<C>],
) {
    let mut hasher1 = Hasher::default();
    let mut hasher2 = Hasher::default();
    let mut hasher3 = Hasher::default();
    let mut hasher4 = Hasher::default();
    let mut hasher5 = Hasher::default();
    let mut hasher6 = Hasher::default();
    let (y0, y1) = simulate_b2a::<_, A, _>(&inputs_0, &inputs_1, cot_alice, cot_bob, &mut hasher1);
    simulate_a2s::<I, A, C, _>(
        inputs_0.len(),
        sqcorr_alice,
        sqcorr_bob,
        &y0,
        &y1,
        &mut hasher2,
        &mut hasher3,
    );
    simulate_ot_verify::<I, A, _>(inputs_1, &cot_bob, 0, &mut hasher4);
    simulate_sqcorr_verify::<I, A, _, _>(
        inputs_0.len(),
        sqcorr_alice,
        sqcorr_bob,
        0,
        &mut hasher5,
        &mut hasher6,
    );
}

trait Config {
    type I: UInt;
    type A: UInt;
    type C: UInt;
    fn gsize() -> &'static [usize];
}

fn msg_gen_benchmark<C: Config>(c: &mut Criterion) {
    let mut group = c.benchmark_group("msg_prepare");
    for gsize in C::gsize() {
        group.bench_with_input(BenchmarkId::from_parameter(gsize), gsize, |b, &gsize| {
            let mut rng = StdRng::from_entropy();
            let data = (0..gsize).map(|_| C::I::rand(&mut rng)).collect::<Vec<_>>();
            b.iter(|| msg_prepare::<C::I, C::A, C::C>(&data));
        });
    }
    drop(group);
    let mut group = c.benchmark_group("transcript_emulation");
    for gsize in C::gsize() {
        group.bench_with_input(BenchmarkId::from_parameter(gsize), gsize, |b, &gsize| {
            let mut rng = StdRng::from_entropy();
            let data = (0..gsize).map(|_| C::I::rand(&mut rng)).collect::<Vec<_>>();
            let (inputs_0, inputs_1, cot_alice, cot_bob, sqcorr_alice, sqcorr_bob) =
                msg_prepare::<C::I, C::A, C::C>(&data);
            b.iter(|| {
                transcript_emulation::<C::I, C::A, C::C>(
                    &inputs_0,
                    &inputs_1,
                    &cot_alice,
                    &cot_bob,
                    &sqcorr_alice,
                    &sqcorr_bob,
                )
            });
        });
    }
}

/// Compare with Prio
struct Bench1 {}
impl Config for Bench1 {
    type I = u32;
    type A = u64;
    type C = u128;

    fn gsize() -> &'static [usize] {
        &[200000, 800000]
    }
}

struct Bench2 {}
impl Config for Bench2 {
    type I = u8;
    type A = u64;
    type C = u128;

    fn gsize() -> &'static [usize] {
        &[62000, 100000, 273000, 300000, 818000]
    }
}

struct Bench3 {}
impl Config for Bench3 {
    type I = u32;
    type A = u64;
    type C = u128;

    fn gsize() -> &'static [usize] {
        &[100000, 300000]
    }
}

criterion_group!(
    msg_gen,
    // msg_gen_benchmark<Bench1>,
    // msg_gen_benchmark<Bench2>,
    msg_gen_benchmark<Bench3>
);
criterion_main!(msg_gen);
