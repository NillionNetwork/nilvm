//! Authentication utilities.

use crate::token::TokenAuthenticator;
use base64::{
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
    Engine,
};
use lru::LruCache;
use node_api::{
    auth::rust::{PublicKey, SignedToken, Token, UserId},
    membership::rust::NodeId,
    ConvertProto, DateTime, Utc,
};
use once_cell::sync::Lazy;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
use tonic::{service::Interceptor, Request, Status};
use user_keypair::{ed25519::Ed25519PublicKey, secp256k1::Secp256k1PublicKey};

const HEADER_NAME_BIN: &str = "x-nillion-token-bin";
const HEADER_NAME_BASE64: &str = "x-nillion-token";
const DEFAULT_LRU_CACHE_CAPACITY: usize = 2048;
const MAX_TOKEN_B64_LENGTH: usize = 512;
static B64_ENGINE: Lazy<GeneralPurpose> = Lazy::new(|| {
    // -bin headers are encoded without padding, but grpc-web headers are b64 encoded using
    // padding, so this engine is indifferent to padding
    let alphabet = &base64::alphabet::STANDARD;
    let config = GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
    GeneralPurpose::new(alphabet, config)
});

/// An interceptor that sends an authentication token in every request.
#[derive(Clone)]
pub struct ClientAuthInterceptor {
    authenticator: TokenAuthenticator,
}

impl ClientAuthInterceptor {
    /// Create a new client interceptor that will use the given authenticator to generate tokens
    /// and tag all requests that go through it with them.
    pub fn new(authenticator: TokenAuthenticator) -> Self {
        Self { authenticator }
    }
}

impl Interceptor for ClientAuthInterceptor {
    fn call(&mut self, mut request: Request<()>) -> tonic::Result<Request<()>> {
        let token =
            self.authenticator.token().map_err(|e| Status::unauthenticated(format!("generating token failed: {e}")))?;
        request.metadata_mut().append_bin(HEADER_NAME_BIN, token);
        Ok(request)
    }
}

/// A tag that indicates a user has been authenticated.
#[derive(Clone)]
pub struct AuthenticatedExtension(pub UserId);

struct ServerInner {
    cache: Mutex<LruCache<Vec<u8>, TokenDetails>>,
    identity: NodeId,
}

/// An interceptor that verifies requests contain an authentication token.
#[derive(Clone)]
pub struct ServerAuthInterceptor {
    inner: Arc<ServerInner>,
}

impl ServerAuthInterceptor {
    /// Construct a new interceptor.
    pub fn new(identity: NodeId) -> Self {
        // SAFETY: this is instantiated in tests.
        #[allow(clippy::unwrap_used)]
        let cache = LruCache::new(NonZeroUsize::new(DEFAULT_LRU_CACHE_CAPACITY).unwrap());
        let cache = Mutex::new(cache);

        Self { inner: Arc::new(ServerInner { cache, identity }) }
    }

    fn authorize(mut request: tonic::Request<()>, identity: UserId) -> tonic::Request<()> {
        request.extensions_mut().insert(AuthenticatedExtension(identity));
        request
    }

    fn verify_signature(message: &[u8], signature: Vec<u8>, public_key: &PublicKey) -> tonic::Result<()> {
        let key: user_keypair::PublicKey = match public_key {
            PublicKey::Ed25519(raw_key) => {
                Ed25519PublicKey::from_bytes(raw_key).map_err(|_| Status::unauthenticated("invalid public key"))?.into()
            }
            PublicKey::Secp256k1(raw_key) => Secp256k1PublicKey::from_bytes(raw_key)
                .map_err(|_| Status::unauthenticated("invalid public key"))?
                .into(),
        };
        key.verify(&signature.into(), message).map_err(|_| Status::unauthenticated("invalid signature"))?;
        Ok(())
    }
}

impl Interceptor for ServerAuthInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> tonic::Result<tonic::Request<()>> {
        // We are okay with unauthenticated requests as any endpoint that requires authentication
        // will look up the authentication tag in the request.
        let Some(b64_token) = extract_token(&request) else { return Ok(request) };
        if b64_token.len() > MAX_TOKEN_B64_LENGTH {
            return Err(Status::unauthenticated(format!("token exceeds max length ({MAX_TOKEN_B64_LENGTH})")));
        }

        let now = Utc::now();
        {
            let mut cache = self.inner.cache.lock().map_err(|_| Status::internal("poisoned lock"))?;
            if let Some(details) = cache.get(b64_token) {
                if details.expires_at < now {
                    return Err(Status::unauthenticated("token is expired"));
                }
                return Ok(Self::authorize(request, details.identity));
            }
        }

        let token_bytes =
            B64_ENGINE.decode(b64_token).map_err(|_| Status::unauthenticated("invalid base64 encoded token"))?;
        let token = SignedToken::try_decode(&token_bytes)
            .map_err(|e| Status::unauthenticated(format!("invalid signed token: {e}")))?;
        Self::verify_signature(&token.serialized_token, token.signature, &token.public_key)?;

        let public_key = token.public_key;
        let token = Token::try_decode(&token.serialized_token)
            .map_err(|e| Status::unauthenticated(format!("invalid token: {e}")))?;
        if token.expires_at < now {
            return Err(Status::unauthenticated("token is expired"));
        }
        if token.target_identity != self.inner.identity {
            return Err(Status::unauthenticated("invalid token target identity"));
        }

        let identity = match public_key {
            PublicKey::Ed25519(public_key) => UserId::from_bytes(public_key),
            PublicKey::Secp256k1(public_key) => UserId::from_bytes(public_key),
        };
        {
            let mut cache = self.inner.cache.lock().map_err(|_| Status::internal("poisoned lock"))?;
            cache.put(b64_token.to_vec(), TokenDetails { expires_at: token.expires_at, identity });
        }
        Ok(Self::authorize(request, identity))
    }
}

fn extract_token(request: &Request<()>) -> Option<&[u8]> {
    match request.metadata().get_bin(HEADER_NAME_BIN) {
        Some(header) => Some(header.as_encoded_bytes()),
        None => request.metadata().get(HEADER_NAME_BASE64).map(|token| token.as_encoded_bytes()),
    }
}

struct TokenDetails {
    identity: UserId,
    expires_at: DateTime<Utc>,
}

/// Allows getting the user id out of an authenticated request.
pub trait AuthenticateRequest {
    /// The identity of the user invoking an RPC endpoint.
    fn user_id(&self) -> tonic::Result<UserId>;
}

impl<T> AuthenticateRequest for Request<T> {
    fn user_id(&self) -> tonic::Result<UserId> {
        let tag = self
            .extensions()
            .get::<AuthenticatedExtension>()
            .ok_or_else(|| Status::permission_denied("permissions denied"))?;
        Ok(tag.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use prost::Message;
    use rstest::rstest;
    use std::{str::FromStr, time::Duration};
    use tonic::metadata::MetadataValue;
    use user_keypair::{ed25519::Ed25519SigningKey, secp256k1::Secp256k1SigningKey, SigningKey};

    fn make_ed25519_authenticator(target_identity: NodeId) -> TokenAuthenticator {
        TokenAuthenticator::new(Ed25519SigningKey::generate().into(), target_identity, Duration::from_secs(60))
    }

    fn make_secp256k1_authenticator(target_identity: NodeId) -> TokenAuthenticator {
        TokenAuthenticator::new(Secp256k1SigningKey::generate().into(), target_identity, Duration::from_secs(60))
    }

    #[test]
    fn request_tagging() {
        let mut interceptor = ClientAuthInterceptor::new(make_ed25519_authenticator(vec![].into()));
        let request = interceptor.call(Request::new(())).expect("intercepting failed");
        assert!(request.metadata().get_bin(HEADER_NAME_BIN).is_some(), "no header set");
    }

    #[rstest]
    #[case::ed25519(make_ed25519_authenticator)]
    #[case::secp256k1(make_secp256k1_authenticator)]
    fn verification(#[case] make_authenticator: fn(NodeId) -> TokenAuthenticator) {
        let identity = NodeId::from(vec![1, 2, 3]);
        let mut interceptor = ClientAuthInterceptor::new(make_authenticator(identity.clone()));
        let request = interceptor.call(Request::new(())).expect("intercepting failed");

        let mut interceptor = ServerAuthInterceptor::new(identity);
        let request = interceptor.call(request).expect("verification failed");
        assert!(request.extensions().get::<AuthenticatedExtension>().is_some());
        request.user_id().expect("no user id");
    }

    #[rstest]
    #[case::ed25519(make_ed25519_authenticator)]
    #[case::secp256k1(make_secp256k1_authenticator)]
    fn verify_invalid_signature(#[case] make_authenticator: fn(NodeId) -> TokenAuthenticator) {
        let identity = NodeId::from(vec![1, 2, 3]);
        let mut interceptor = ServerAuthInterceptor::new(identity.clone());

        // Change every byte one by one and ensure all the modified token fail to authenticate.
        let token = make_authenticator(identity).token().unwrap().to_bytes().unwrap();
        for index in 0..token.len() {
            let mut token = token.to_vec();
            token[index] = token[index].wrapping_add(1);

            let mut request = Request::new(());
            request.metadata_mut().append_bin(HEADER_NAME_BIN, MetadataValue::from_bytes(&token));
            interceptor.call(request).expect_err("verification succeeded");
        }
    }

    #[test]
    fn base64_token_verification() {
        let identity = NodeId::from(vec![1, 2, 3]);
        let expires_at = Utc::now() + Duration::from_secs(60);
        let token = Token { nonce: [1; 32], target_identity: identity.clone(), expires_at };
        let serialized_token = token.into_proto().encode_to_vec();

        let key = Secp256k1SigningKey::try_from_seed("test").unwrap();
        let signing_key = SigningKey::from(key);
        let public_key: [u8; 33] = signing_key.public_key().as_bytes().try_into().unwrap();
        let signature: Vec<u8> = signing_key.sign(&serialized_token).into();
        let signed = SignedToken { serialized_token, public_key: PublicKey::Secp256k1(public_key), signature };

        let serialized_signed_token = signed.into_proto().encode_to_vec();
        let base64_token = base64::engine::general_purpose::STANDARD.encode(serialized_signed_token);

        let mut interceptor = ServerAuthInterceptor::new(identity);
        let mut request = Request::new(());
        request.metadata_mut().append(HEADER_NAME_BASE64, MetadataValue::from_str(&base64_token).unwrap());

        let request = interceptor.call(request).expect("base64 token verification failed");
        assert!(request.extensions().get::<AuthenticatedExtension>().is_some());
    }

    #[rstest]
    #[case::no_pads("aGkgbW9t")]
    #[case::one_pad("aGkgbW9tISE=")]
    #[case::two_pads("aGkgbW9tIQ==")]
    fn b64_decoding(#[case] input: &str) {
        B64_ENGINE.decode(input.as_bytes()).expect("failed to decode");
    }
}
