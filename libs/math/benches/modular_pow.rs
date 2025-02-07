use criterion::{black_box, criterion_group, criterion_main, Criterion};
use crypto_bigint::{U256, U64};
use math_lib::modular::{power::Power, ModularNumber, ModularPow, U256SafePrime, U64SafePrime};

fn run_exp_mod_bench(c: &mut Criterion) {
    let generator = ModularNumber::<U256SafePrime>::GENERATOR;
    let exponent = U256::from_u64(10293817823993_u64);
    c.bench_function("exp_mod 256", |b| b.iter(|| generator.exp_mod(black_box(&exponent))));
}

fn run_power_bench(c: &mut Criterion) {
    let generator = ModularNumber::<U256SafePrime>::GENERATOR;
    let power = Power::new(generator);
    let exponent = U256::from_u64(10293817823993_u64);
    c.bench_function("power 256", |b| b.iter(|| power.exp(black_box(&exponent))));
}

fn run_exp_mod_bench_64(c: &mut Criterion) {
    let generator = ModularNumber::<U64SafePrime>::GENERATOR;
    let exponent = U64::from_u64(15192310293817823993_u64);
    c.bench_function("exp_mod 64", |b| b.iter(|| generator.exp_mod(black_box(&exponent))));
}

fn run_power_bench_64(c: &mut Criterion) {
    let generator = ModularNumber::<U64SafePrime>::GENERATOR;
    let power = Power::new(generator);
    let exponent = U64::from_u64(15192310293817823993_u64);
    c.bench_function("power 64", |b| b.iter(|| power.exp(black_box(&exponent))));
}

criterion_group!(
    name = static_exp_mod_bench;
    config = Criterion::default();
    targets = run_exp_mod_bench
);

criterion_group!(
    name = static_power_bench;
    config = Criterion::default();
    targets = run_power_bench
);

criterion_group!(
    name = static_exp_mod_bench_64;
    config = Criterion::default();
    targets = run_exp_mod_bench_64
);

criterion_group!(
    name = static_power_bench_64;
    config = Criterion::default();
    targets = run_power_bench_64
);

criterion_main!(static_exp_mod_bench, static_power_bench, static_exp_mod_bench_64, static_power_bench_64);
