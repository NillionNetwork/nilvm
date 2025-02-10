use anyhow::{Context, Result};
use clap::Parser;
use math_lib::{
    impl_boxed_from_encoded_safe_prime,
    modular::{ModularNumber, SafePrime},
    ring::RingTuple,
};
use num_bigint::BigUint;
use shamir_sharing::secret_sharer::{SecretSharer, ShamirSecretSharer};
use share_reconstructor::{
    config::{Config, SharesConfig},
    reconstruct::Reconstructor,
};
use std::marker::PhantomData;

#[derive(Parser, Debug)]
#[clap(name = "blinding-factor-debugger")]
struct Options {
    /// The path to the config file
    config_path: String,
}

trait Runner {
    fn reconstruct(&self, reconstructor: Reconstructor, shares: SharesConfig) -> anyhow::Result<()>;
}

#[derive(Default)]
struct PrimeRunner<T>(PhantomData<T>);

impl<T> Runner for PrimeRunner<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SecretSharer<ModularNumber<T>, Secret = ModularNumber<T>>
        + SecretSharer<RingTuple<T::SophiePrime>, Secret = ModularNumber<T::SemiPrime>>,
{
    fn reconstruct(&self, reconstructor: Reconstructor, shares: SharesConfig) -> anyhow::Result<()> {
        let secret = match shares {
            SharesConfig::PrimeField { shares } => {
                reconstructor.reconstruct::<T, _, _>(shares).map(|v| BigUint::from(&v))
            }
            SharesConfig::SemiField { shares } => {
                reconstructor.reconstruct::<T, _, _>(shares).map(|v| BigUint::from(&v))
            }
        }?;
        println!("Secret recovered is: {secret}");
        Ok(())
    }
}

impl_boxed_from_encoded_safe_prime!(PrimeRunner, Runner);

fn main() -> Result<()> {
    let options = Options::parse();
    let config = Config::load(&options.config_path)?;
    let reconstructor = Reconstructor;

    let prime = config.prime;
    println!("Recovering secret using prime {prime:?}");
    let runner = Box::<dyn Runner>::try_from(&prime).context("invalid prime")?;
    runner.reconstruct(reconstructor, config.shares).context("reconstruction failed")
}
