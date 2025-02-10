use basic_types::PartyId;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use math_lib::{
    modular::{ModularNumber, SafePrime, U256SafePrime, U256SemiPrime, U256SophiePrime},
    ring::RingTuple,
};
use shamir_sharing::{
    protocol::PolyDegree,
    secret_sharer::{PartyShares, SecretSharer, ShamirSecretSharer},
};

const SECRET_VALUE: u64 = 15130512518_u64;

fn build_shamir<T: SafePrime>(party_count: usize) -> ShamirSecretSharer<T> {
    let parties: Vec<_> = (0..party_count).map(PartyId::from).collect();
    ShamirSecretSharer::new(parties[0].clone(), 1, parties).expect("shamir construction failed")
}

fn bench_generate_shares<R, T, S>(bench_name: &str, secret: S, c: &mut Criterion)
where
    T: SafePrime,
    ShamirSecretSharer<T>: SecretSharer<R, Secret = S>,
{
    let shamir = build_shamir::<T>(5);
    c.bench_function(bench_name, |b| {
        b.iter(|| {
            let _: PartyShares<R> =
                shamir.generate_shares(black_box(&secret), PolyDegree::T).expect("generate shares failed");
        });
    });
}

fn bench_recover<T, S>(bench_name: &str, shamir: ShamirSecretSharer<T>, shares: PartyShares<S>, c: &mut Criterion)
where
    ShamirSecretSharer<T>: SecretSharer<S>,
    T: SafePrime,
    S: Clone,
{
    c.bench_function(bench_name, |b| {
        b.iter(|| {
            shamir.recover(black_box(shares.clone())).expect("recover failed");
        });
    });
}

fn bench_generate_shares_modular(c: &mut Criterion) {
    let secret = ModularNumber::<U256SafePrime>::from_u64(SECRET_VALUE);
    bench_generate_shares::<ModularNumber<U256SafePrime>, _, _>("generate shares (modular)", secret, c);
}

fn bench_generate_shares_ring_tuple(c: &mut Criterion) {
    let secret = ModularNumber::<U256SemiPrime>::from_u64(SECRET_VALUE);
    bench_generate_shares::<RingTuple<U256SophiePrime>, U256SafePrime, _>("generate shares (ring ruple)", secret, c);
}

fn bench_recover_modular(c: &mut Criterion) {
    let secret = ModularNumber::<U256SafePrime>::from_u64(SECRET_VALUE);
    let shamir = build_shamir(5);
    let shares: PartyShares<ModularNumber<U256SafePrime>> =
        shamir.generate_shares(&secret, PolyDegree::T).expect("generate shares failed");
    bench_recover("recover (modular)", shamir, shares, c);
}

fn bench_recover_ring_tuple(c: &mut Criterion) {
    let secret = ModularNumber::<U256SemiPrime>::from_u64(SECRET_VALUE);
    let shamir = build_shamir::<U256SafePrime>(5);
    let shares: PartyShares<RingTuple<U256SophiePrime>> =
        shamir.generate_shares(&secret, PolyDegree::T).expect("generate shares failed");
    bench_recover("recover (ring tuple)", shamir, shares, c);
}

criterion_group!(
    name = generate_shares;
    config = Criterion::default();
    targets = bench_generate_shares_modular, bench_generate_shares_ring_tuple
);

criterion_group!(
    name = recover;
    config = Criterion::default();
    targets = bench_recover_modular, bench_recover_ring_tuple
);

criterion_main!(generate_shares, recover);
