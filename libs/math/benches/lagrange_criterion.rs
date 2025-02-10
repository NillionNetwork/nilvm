use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_lib::decoders::lagrange::lagrange_test::lagrange_bench;
use std::time::Duration;

fn run_lagrange_bench(c: &mut Criterion) {
    c.bench_function("32-degree polynomial interpolation u64 100 secrets", |b| {
        b.iter(|| lagrange_bench(black_box(100)))
    });
}

criterion_group!(
    name = random_lagrange_bench;
    config = Criterion::default().significance_level(0.1).sample_size(10).measurement_time(Duration::from_secs(2));
    targets = run_lagrange_bench
);

criterion_main!(random_lagrange_bench);
