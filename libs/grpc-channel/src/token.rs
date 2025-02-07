//! gRPC tokens.

use node_api::{
    auth::rust::{PublicKey, SignedToken, Token},
    membership::rust::NodeId,
    ConvertProto, DateTime, Message, Utc,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tonic::metadata::{Binary, MetadataValue};

pub use user_keypair::SigningKey;

#[derive(Clone)]
struct LatestToken {
    token: MetadataValue<Binary>,
    renew_at: DateTime<Utc>,
}

struct Inner {
    signing_key: SigningKey,
    public_key: PublicKey,
    expiration: Duration,
    renew_threshold: Duration,
    target_identity: NodeId,
}

/// An authenticator that generates tokens on demand signed by a private key.
///
/// Tokens are only regenerated if they're about to expire.
#[derive(Clone)]
pub struct TokenAuthenticator {
    inner: Arc<Inner>,
    token: Arc<Mutex<LatestToken>>,
}

impl TokenAuthenticator {
    /// Construct a new authenticator that will use the given ed25519 key to generate tokens.
    pub fn new(signing_key: SigningKey, target_identity: NodeId, expiration: Duration) -> Self {
        let public_key = match signing_key.public_key() {
            user_keypair::PublicKey::Ed25519(key) => PublicKey::Ed25519(*key.as_bytes()),
            user_keypair::PublicKey::Secp256k1(key) => {
                // SAFETY: this is the actual length and tests validate this is the case.
                #[allow(clippy::expect_used)]
                let key: [u8; 33] = key.as_bytes().try_into().expect("not 33 bytes long");
                PublicKey::Secp256k1(key)
            }
        };
        // Create a dummy token that's expired so we regenerate it on first use.
        let token = LatestToken { token: MetadataValue::from_bytes(b""), renew_at: DateTime::UNIX_EPOCH };
        let token = Arc::new(Mutex::new(token));
        let renew_threshold = expiration.as_secs() as f64 * 0.80;
        let renew_threshold = Duration::from_secs(renew_threshold as u64);
        let inner = Inner { signing_key, public_key, expiration, renew_threshold, target_identity }.into();
        Self { inner, token }
    }

    /// Get a valid authentication token.
    ///
    /// This will only re-generate a token if the latest one generated is close to expiring.
    ///
    /// The returned token must be used immediately on gRPC requests as it could otherwise expire
    /// before it's used.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn token(&self) -> Result<MetadataValue<Binary>, GenerateTokenError> {
        let now = Utc::now();
        let mut token = self.token.lock().map_err(|_| GenerateTokenError("internal error: locking"))?;
        if token.renew_at < now {
            let serialized_token = Token {
                nonce: rand::random(),
                target_identity: self.inner.target_identity.clone(),
                expires_at: now + self.inner.expiration,
            }
            .into_proto()
            .encode_to_vec();

            let signature = self.inner.signing_key.sign(&serialized_token).into();
            let new_token = SignedToken { serialized_token, public_key: self.inner.public_key.clone(), signature };
            token.token = MetadataValue::from_bytes(&new_token.into_proto().encode_to_vec());
            token.renew_at = now + self.inner.renew_threshold;
        }
        Ok(token.token.clone())
    }
}

/// An error during the generation of a token.
#[derive(Debug, thiserror::Error)]
#[error("error generating token: {0}")]
pub struct GenerateTokenError(&'static str);

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;
    use user_keypair::{ed25519::Ed25519SigningKey, secp256k1::Secp256k1SigningKey};

    #[rstest]
    #[case::ed25519(Ed25519SigningKey::generate().into())]
    #[case::secp256k1(Secp256k1SigningKey::generate().into())]
    fn token_generation(#[case] key: SigningKey) {
        let now = Utc::now();
        let authenticator = TokenAuthenticator::new(key.clone(), vec![1, 2, 3].into(), Duration::from_secs(60));
        let token = authenticator.token().expect("failed to generate token");
        // We should get the same token if we call it twice.
        assert_eq!(token, authenticator.token().unwrap());

        let token = token.to_bytes().expect("invalid bytes");
        let token = SignedToken::try_decode(&token).expect("singed token decoding failed");
        let inner = Token::try_decode(&token.serialized_token).expect("inner token decoding failed");
        let expires_at = inner.expires_at;
        assert!(expires_at > now + Duration::from_secs(50), "expiration is too short: {now} vs {expires_at}");
        assert!(expires_at < now + Duration::from_secs(70), "expiration is too long: {now} vs {expires_at}");
    }
}
