use bridge::BlackBox;
use client_baseline::protocol::Client;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use crypto_primitives::uint::UInt;
use rand::{rngs::StdRng, SeedableRng};
fn run_msg_gen<I: UInt>(data: &[I]) {
    let mut rng = StdRng::from_entropy();
    let client = Client::<I>::new(data, &mut rng);
    client.drop_into_black_box();
}

trait Config {
    const NAME: &'static str;
    type I: UInt;

    fn gsize() -> &'static [usize];
}

fn msg_gen_benchmark<C: Config>(c: &mut Criterion) {
    let mut group = c.benchmark_group(C::NAME);
    for gsize in C::gsize() {
        group.bench_with_input(BenchmarkId::from_parameter(gsize), gsize, |b, &gsize| {
            let mut rng = StdRng::from_entropy();
            let data = (0..gsize).map(|_| C::I::rand(&mut rng)).collect::<Vec<_>>();
            b.iter(|| run_msg_gen::<C::I>(&data));
        });
    }
}

/// Compare with Prio
struct Table4 {}
impl Config for Table4 {
    const NAME: &'static str = "table4";
    type I = u32;

    fn gsize() -> &'static [usize] {
        &[200000, 800000]
    }
}

criterion_group!(msg_gen, msg_gen_benchmark<Table4>);
criterion_main!(msg_gen);
