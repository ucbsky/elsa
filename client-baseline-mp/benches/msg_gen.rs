use bridge::BlackBox;

use client_baseline_mp::data_prep::prepare_message;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use crypto_primitives::uint::UInt;
use prio::field::{Field64, FieldElement};
use rand::{rngs::StdRng, SeedableRng};

fn run_msg_gen<I: UInt, F: FieldElement>(data: &[I]) {
    prepare_message::<_, Field64>(&data).drop_into_black_box()
}

trait Config {
    type I: UInt;
    type F: FieldElement;
    fn gsize() -> &'static [usize];
}

fn msg_gen_benchmark<C: Config>(c: &mut Criterion) {
    let mut group = c.benchmark_group("msg_gen_u32");
    group.sample_size(10);
    for gsize in C::gsize() {
        group.bench_with_input(BenchmarkId::from_parameter(gsize), gsize, |b, &gsize| {
            let mut rng = StdRng::from_entropy();
            let data = (0..gsize).map(|_| C::I::rand(&mut rng)).collect::<Vec<_>>();
            b.iter(|| run_msg_gen::<C::I, C::F>(&data));
        });
    }
}

/// Compare with ELSA Po2 MP
struct Table2 {}
impl Config for Table2 {
    type I = u32;
    type F = Field64;

    fn gsize() -> &'static [usize] {
        &[100000, 300000]
    }
}

criterion_group!(msg_gen, msg_gen_benchmark<Table2>);
criterion_main!(msg_gen);
