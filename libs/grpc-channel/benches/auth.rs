use criterion::{black_box, criterion_group, criterion_main, Criterion};
use grpc_channel::{
    auth::{ClientAuthInterceptor, ServerAuthInterceptor},
    token::TokenAuthenticator,
};
use node_api::membership::rust::NodeId;
use std::time::Duration;
use tonic::{
    metadata::{Binary, MetadataValue},
    service::Interceptor,
    Request,
};
use user_keypair::{ed25519::Ed25519SigningKey, secp256k1::Secp256k1SigningKey, SigningKey};

fn generate_token(authenticator: &TokenAuthenticator) -> MetadataValue<Binary> {
    authenticator.token().expect("generating token failed")
}

fn verify_token(
    client_interceptor: &mut ClientAuthInterceptor,
    server_interceptor: &mut ServerAuthInterceptor,
) -> Request<()> {
    let request = client_interceptor.call(Request::new(())).expect("failed to create request");
    server_interceptor.call(request).expect("verification failed")
}

fn benchmark_generate(c: &mut Criterion, id: &str, key: SigningKey) {
    let authenticator = TokenAuthenticator::new(key, vec![0; 16].into(), Duration::from_secs(60));
    c.bench_function(id, |b| b.iter(|| generate_token(black_box(&authenticator))));
}

fn benchmark_generate_token_ed25519(c: &mut Criterion) {
    let key = Ed25519SigningKey::generate().into();
    benchmark_generate(c, "generate token ed25519", key);
}

fn benchmark_generate_token_secp256k1(c: &mut Criterion) {
    let key = Secp256k1SigningKey::generate().into();
    benchmark_generate(c, "generate token secp256k1", key);
}

fn benchmark_verify_token(c: &mut Criterion, id: &str, key: SigningKey) {
    let identity = NodeId::from(vec![0; 16]);
    let authenticator = TokenAuthenticator::new(key, identity.clone(), Duration::from_secs(60));
    let mut client_interceptor = ClientAuthInterceptor::new(authenticator);
    let mut server_interceptor = ServerAuthInterceptor::new(identity);
    c.bench_function(id, |b| {
        b.iter(|| verify_token(black_box(&mut client_interceptor), black_box(&mut server_interceptor)))
    });
}

fn benchmark_verify_token_ed25519(c: &mut Criterion) {
    let key = Ed25519SigningKey::generate().into();
    benchmark_verify_token(c, "verify token ed25519", key);
}

fn benchmark_verify_token_secp256k1(c: &mut Criterion) {
    let key = Secp256k1SigningKey::generate().into();
    benchmark_verify_token(c, "verify token secp256k1", key);
}

criterion_group!(ed25519, benchmark_generate_token_ed25519, benchmark_verify_token_ed25519);
criterion_group!(secp256k1, benchmark_generate_token_secp256k1, benchmark_verify_token_secp256k1);
criterion_main!(ed25519, secp256k1);
