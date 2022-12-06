use bridge::BlackBox;
use client_mp_po2::protocol::Client;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use crypto_primitives::{malpriv::client::simulate_ot_verify, uint::UInt};
use rand::{rngs::StdRng, SeedableRng};
use sha2::Sha256;

type Hasher = Sha256;
fn run_msg_gen<I: UInt, A: UInt>(data: &[I]) {
    let mut rng = StdRng::from_entropy();
    let client = Client::prepare_phase1::<I, _, _>(data, &mut rng, || Hasher::default());
    let chi_seed = 0;
    let mut hasher = Hasher::default();
    simulate_ot_verify::<I, A, Hasher>(
        &client.prepared_message_b.0.inputs_1,
        &client.prepared_message_b.0.cot,
        chi_seed,
        &mut hasher,
    );
    hasher.drop_into_black_box();
}

trait Config {
    type I: UInt;
    type A: UInt;
    fn gsize() -> &'static [usize];
}

fn msg_gen_benchmark<C: Config>(c: &mut Criterion) {
    let mut group = c.benchmark_group("msg_gen_u32");
    for gsize in C::gsize() {
        group.bench_with_input(BenchmarkId::from_parameter(gsize), gsize, |b, &gsize| {
            let mut rng = StdRng::from_entropy();
            let data = (0..gsize).map(|_| C::I::rand(&mut rng)).collect::<Vec<_>>();
            b.iter(|| run_msg_gen::<C::I, C::A>(&data));
        });
    }
}

/// Compare with Prio
struct Table2 {}
impl Config for Table2 {
    type I = u32;
    type A = u64;

    fn gsize() -> &'static [usize] {
        &[100000, 500000]
    }
}

criterion_group!(msg_gen, msg_gen_benchmark<Table2>);
criterion_main!(msg_gen);
