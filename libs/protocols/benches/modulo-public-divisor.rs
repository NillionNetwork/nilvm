use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use math_lib::modular::{SafePrime, U256SafePrime};
use protocols::{
    division::modulo_public_divisor::offline::protocol::PrepModuloProtocol,
    simulator::symmetric::SymmetricProtocolSimulator,
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

struct Config {
    polynomial_degree: u64,
    element_count: usize,
    kappa: usize,
    k: usize,
    network_size: usize,
}

impl Default for Config {
    // The default values here are not special but are just sane values.
    fn default() -> Self {
        // The degree of the polynomials being used to hide secrets. This parameter can be tweaked
        // but tweaks
        // need to go in tandem with `network_size`.
        let polynomial_degree = 1;

        // The number of elements.
        let element_count = 1;

        // The statistical security parameter.
        let kappa = 40;

        // The size of elements.
        let k = 10;

        // The number of parties in the network.
        let network_size = 5;

        Self { polynomial_degree, element_count, kappa, k, network_size }
    }
}

impl Config {
    fn prepare<T: SafePrime>(&self) -> impl Fn()
    where
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let max_rounds = 100;
        let simulator = SymmetricProtocolSimulator::new(self.network_size, max_rounds).with_diagnostics(false);
        let protocol = PrepModuloProtocol::<T>::new(self.element_count, self.polynomial_degree, self.kappa, self.k);
        move || {
            simulator.run_protocol(black_box(&protocol)).expect("protocol execution failed");
        }
    }
}

// Defines a macro that takes:
// * A benchmark name.
// * The prime number to use.
// * The property in `Config` that will be tested.
// * The values that we want to give that property.
macro_rules! benchmark_property {
    ($bench_name:ident, $prime:ty, $prop:ident, $values:expr) => {
        fn $bench_name(c: &mut Criterion) {
            let mut config = Config::default();
            let mut group = c.benchmark_group(stringify!($prop));
            for value in $values {
                group.bench_with_input(BenchmarkId::from_parameter(value), &value, |b, &value| {
                    config.$prop = value;
                    let runner = config.prepare::<$prime>();
                    b.iter(runner);
                });
            }
        }
    };
}

benchmark_property!(bench_element_count, U256SafePrime, element_count, [10, 100]);
benchmark_property!(bench_network_size, U256SafePrime, network_size, [5, 10]);

criterion_group!(
    name = benches;
    config = Criterion::default().significance_level(0.1).sample_size(20);
    targets = bench_element_count, bench_network_size
);

criterion_main!(benches);
