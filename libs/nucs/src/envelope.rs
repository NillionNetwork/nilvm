use crate::token::{NucToken, ProofHash};
use base64::{display::Base64Display, prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use k256::ecdsa::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use signature::Verifier;
use std::{iter, marker::PhantomData};

const DEFAULT_MAX_RAW_TOKEN_SIZE: usize = 1024 * 10;

/// An envelope for a NUC token.
///
/// This contains the NUC token itself along with all of its proofs.
#[derive(Clone, Debug)]
pub struct NucTokenEnvelope<T = SignaturesUnvalidated> {
    token: DecodedNucToken,
    proofs: Vec<DecodedNucToken>,
    _unused: PhantomData<T>,
}

impl NucTokenEnvelope<SignaturesUnvalidated> {
    /// Decode a NUC token.
    ///
    /// This performs no integrity checks, and instead only ensures the token and its proofs are well formed.
    pub fn decode(s: &str) -> Result<Self, NucEnvelopeParseError> {
        NucTokenEnvelopeDecoder::default().decode(s)
    }

    /// Encode this envelope.
    pub fn encode(&self) -> String {
        let tokens = iter::once(&self.token).chain(&self.proofs);
        let mut output = String::new();
        for (index, token) in tokens.enumerate() {
            if index > 0 {
                output.push('/');
            }
            output.push_str(&token.to_jwt());
        }
        output
    }

    /// Validate the signature of this token and all of its proofs.
    pub fn validate_signatures(self) -> Result<NucTokenEnvelope<SignaturesValidated>, InvalidSignature> {
        let tokens = iter::once(&self.token).chain(self.proofs.iter());
        for token in tokens {
            token.validate_signature()?;
        }
        Ok(NucTokenEnvelope { token: self.token, proofs: self.proofs, _unused: PhantomData })
    }
}

impl<T> NucTokenEnvelope<T> {
    /// Split this envelope into its parts.
    pub fn into_parts(self) -> (DecodedNucToken, Vec<DecodedNucToken>) {
        (self.token, self.proofs)
    }

    /// Get the token in this envelope.
    pub fn token(&self) -> &DecodedNucToken {
        &self.token
    }

    /// Get the proofs in this envelope.
    pub fn proofs(&self) -> &[DecodedNucToken] {
        &self.proofs
    }
}

/// A NUC token envelope decoder.
#[derive(Debug)]
pub struct NucTokenEnvelopeDecoder {
    /// The maximum raw token size, in bytes.
    pub max_raw_token_size: usize,
}

impl Default for NucTokenEnvelopeDecoder {
    fn default() -> Self {
        Self { max_raw_token_size: DEFAULT_MAX_RAW_TOKEN_SIZE }
    }
}

impl NucTokenEnvelopeDecoder {
    /// Decode a NUC.
    pub fn decode(&self, s: &str) -> Result<NucTokenEnvelope, NucEnvelopeParseError> {
        if s.len() > self.max_raw_token_size {
            return Err(NucEnvelopeParseError::NucTooLarge(self.max_raw_token_size));
        }
        let mut jwts = s.split('/');

        // The first element is the actual token, the rest of them are its proofs
        let token_jwt = jwts.next().ok_or(NucEnvelopeParseError::NoInput)?;
        let token = DecodedNucToken::from_jwt(token_jwt)?;
        let mut proofs = Vec::new();
        for jwt in jwts {
            let proof = DecodedNucToken::from_jwt(jwt)?;
            proofs.push(proof);
        }
        Ok(NucTokenEnvelope { token, proofs, _unused: PhantomData })
    }
}

/// A tag type to indicate an envelope has had its signatures validated.
#[derive(Clone, Debug)]
pub struct SignaturesValidated;

/// A tag type to indicate an envelope has not had its signatures validated.
#[derive(Clone, Debug)]
pub struct SignaturesUnvalidated;

/// An error when parsing a NUC envelope.
#[derive(Debug, thiserror::Error)]
pub enum NucEnvelopeParseError {
    #[error("empty input")]
    NoInput,

    #[error("NUC is larger than max allowed: {0} bytes")]
    NucTooLarge(usize),

    #[error("invalid NUC JWT: {0}")]
    Jwt(#[from] NucJwtParseError),
}

/// A raw NUC token.
///
/// This contains the raw parts that were serialized from a JWT and is used to have access to the
/// original unmodified
#[derive(Clone, Debug)]
pub struct RawNucToken {
    pub(crate) header: Vec<u8>,
    pub(crate) payload: Vec<u8>,
    pub(crate) signature: Vec<u8>,
}

impl RawNucToken {
    fn from_jwt(s: &str) -> Result<Self, NucJwtParseError> {
        let mut chunks = s.splitn(3, '.');
        let header = Self::parse_base64_next(&mut chunks, "header")?;
        let payload = Self::parse_base64_next(&mut chunks, "token")?;
        let signature = chunks.next().ok_or(NucJwtParseError::MissingComponent("signature"))?;
        let signature: Vec<u8> = from_base64(signature).map_err(|e| NucJwtParseError::Base64("signature", e))?;
        Ok(Self { header, payload, signature })
    }

    fn to_jwt(&self) -> String {
        let header = Base64Display::new(&self.header, &BASE64_URL_SAFE_NO_PAD);
        let payload = Base64Display::new(&self.payload, &BASE64_URL_SAFE_NO_PAD);
        let signature = Base64Display::new(&self.signature, &BASE64_URL_SAFE_NO_PAD);
        format!("{header}.{payload}.{signature}")
    }

    fn parse_base64_next<'a, I>(iter: &mut I, component: &'static str) -> Result<Vec<u8>, NucJwtParseError>
    where
        I: Iterator<Item = &'a str>,
    {
        let next = iter.next().ok_or(NucJwtParseError::MissingComponent(component))?;
        from_base64(next).map_err(|e| NucJwtParseError::Base64(component, e))
    }
}

/// An error when parsing a NUC in JWT format.
#[derive(Debug, thiserror::Error)]
pub enum NucJwtParseError {
    #[error("no {0} component in jwt")]
    MissingComponent(&'static str),

    #[error("invalid base64 found on {0}: {1}")]
    Base64(&'static str, base64::DecodeError),

    #[error("invalid JSON on {0}: {1}")]
    Json(&'static str, serde_json::Error),
}

/// A decoded NUC token.
///
/// This is a wrapper over a NUC token along with its raw pieces to be able to re-serialize it
/// later on without altering its contents.
#[derive(Clone, Debug)]
pub struct DecodedNucToken {
    pub(crate) raw: RawNucToken,
    pub(crate) token: NucToken,
}

impl DecodedNucToken {
    fn from_raw(raw: RawNucToken) -> Result<Self, NucJwtParseError> {
        // Validate the JWT header
        let _header: JwtHeader = serde_json::from_slice(&raw.header).map_err(|e| NucJwtParseError::Json("token", e))?;
        let token = serde_json::from_slice(&raw.payload).map_err(|e| NucJwtParseError::Json("token", e))?;
        Ok(Self { raw, token })
    }

    fn from_jwt(s: &str) -> Result<Self, NucJwtParseError> {
        let raw = RawNucToken::from_jwt(s)?;
        Self::from_raw(raw)
    }

    pub(crate) fn to_jwt(&self) -> String {
        self.raw.to_jwt()
    }

    /// Compute this token's hash.
    pub(crate) fn compute_hash(&self) -> ProofHash {
        let input = self.to_jwt();
        let hash = Sha256::digest(&input);
        ProofHash(hash.into())
    }

    /// Validate the signature in this token.
    pub(crate) fn validate_signature(&self) -> Result<(), InvalidSignature> {
        let header = Base64Display::new(&self.raw.header, &BASE64_URL_SAFE_NO_PAD);
        let payload = Base64Display::new(&self.raw.payload, &BASE64_URL_SAFE_NO_PAD);
        let input = format!("{header}.{payload}");

        // SAFETY: we know there's at least 3 dots because we pulled a signed token successfully.
        let verifying_key = VerifyingKey::from_sec1_bytes(&self.token.issuer.public_key)
            .map_err(|_| InvalidSignature::IssuerPublicKey)?;
        let k256_signature =
            Signature::try_from(self.raw.signature.as_slice()).map_err(|_| InvalidSignature::Signature)?;
        verifying_key.verify(input.as_bytes(), &k256_signature).map_err(|_| InvalidSignature::Signature)?;
        Ok(())
    }

    /// Get the NUC token.
    pub fn token(&self) -> &NucToken {
        &self.token
    }
}

/// An error during the verification of a token signature.
#[derive(Debug, thiserror::Error)]
pub enum InvalidSignature {
    #[error("invalid issuer public key")]
    IssuerPublicKey,

    #[error("invalid signature")]
    Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JwtHeader {
    #[serde(rename = "alg")]
    pub(crate) algorithm: JwtAlgorithm,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum JwtAlgorithm {
    Es256k,
}

pub(crate) fn from_base64(input: &str) -> Result<Vec<u8>, base64::DecodeError> {
    BASE64_URL_SAFE_NO_PAD.decode(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::{to_base64, NucTokenBuilder},
        policy,
        token::Did,
    };
    use k256::SecretKey;
    use rstest::rstest;
    use serde_json::json;

    // this is a valid token without any proofs
    const VALID_TOKEN: &str = "eyJhbGciOiJFUzI1NksifQ.eyJpc3MiOiJkaWQ6bmlsOjAyMjZhNGQ0YTRhNWZhZGUxMmM1ZmYwZWM5YzQ3MjQ5ZjIxY2Y3N2EyMDI3NTFmOTU5ZDVjNzc4ZjBiNjUyYjcxNiIsImF1ZCI6ImRpZDpuaWw6YmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwic3ViIjoiZGlkOm5pbDpjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2MiLCJjbWQiOiIvbmlsL2RiL3JlYWQiLCJhcmdzIjp7ImZvbyI6NDJ9LCJub25jZSI6IjAxMDIwMyIsInByZiI6WyJjOTA0YzVhMWFiMzY5YWVhMWI0ZDlkMTkwMmE0NmU2ZWY5NGFhYjk2OTY0YmI1MWQ2MWE2MWIwM2UyM2Q1ZGZmIl19.ufDYxqoSVNVETrVKReu0h_Piul5c6RoC_VnGGLw04mkyn2OMrtQjK92sGXNHCjlp7T9prIwxX14ZB_N3gx7hPg";

    #[test]
    fn decoding() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let encoded = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::nil([0xbb; 33]))
            .subject(Did::nil([0xcc; 33]))
            .command(["nil", "db", "read"])
            .nonce([1, 2, 3])
            .build(&key.into())
            .expect("build failed");
        let envelope = NucTokenEnvelope::decode(&encoded).expect("decode failed");
        envelope.validate_signatures().expect("signature validation failed");
    }

    #[test]
    fn invalid_signature() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let token = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::nil([0xbb; 33]))
            .subject(Did::nil([0xcc; 33]))
            .command(["nil", "db", "read"])
            .nonce([1, 2, 3])
            .build(&key.into())
            .expect("build failed");
        let (base, signature) = token.rsplit_once(".").unwrap();

        // Change every byte in the signature and make sure validation fails every time.
        let signature = from_base64(signature).unwrap();
        for index in 0..signature.len() {
            let mut signature = signature.clone();
            signature[index] = signature[index].wrapping_add(1);
            let signature = to_base64(&signature);
            let token = format!("{base}.{signature}");
            let envelope = NucTokenEnvelope::decode(&token).expect("decode failed");
            envelope.validate_signatures().expect_err("validation succeeded");
        }
    }

    #[rstest]
    #[case::dot(".")]
    #[case::malformed_bas64(".&&&")]
    #[case::malformed_json(".eyJmb28iOiJiYXIifQ")]
    #[case::invalid_json(".eyJmb28iOiJiYXIi")]
    #[case::emoji(".🚀")]
    #[case::b64_emoji(".8J-agA==")]
    fn invalid_suffixes(#[case] suffix: &str) {
        // append the suffix where the proof array would go
        let input = format!("{VALID_TOKEN}{suffix}");
        NucTokenEnvelope::decode(&input).expect_err("decode succeeded");
    }

    #[test]
    fn token_serde() {
        // Decode a valid raw token.
        let token = RawNucToken::from_jwt(VALID_TOKEN).expect("decoding failed");

        // Split the raw token in its base64 decoded chunks
        let mut chunks = VALID_TOKEN.split('.').map(from_base64).collect::<Result<Vec<_>, _>>().unwrap().into_iter();
        let header = chunks.next().unwrap();
        let payload = chunks.next().unwrap();
        let signature = chunks.next().unwrap();

        assert_eq!(header, token.header);
        assert_eq!(payload, token.payload);
        assert_eq!(signature, token.signature);

        // Serialize the individual pieces of the decoded token.
        let serialized_header = to_base64(&token.header);
        let serialized_payload = to_base64(&token.payload);
        let serialized_signature = to_base64(token.signature);

        // Reconstruct back the token from its serialized pieces.
        let reconstructed = format!("{serialized_header}.{serialized_payload}.{serialized_signature}",);

        // Ensure the reconstructed token is still valid, meaning anyone can verify the signature
        // for a proof in an envelope.
        NucTokenEnvelope::decode(&reconstructed)
            .expect("reconstruction failed")
            .validate_signatures()
            .expect("signature validation failed");
    }

    #[test]
    fn deep_nesting() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let mut policy = policy::op::eq(".foo", json!(42));
        for _ in 0..128 {
            policy = policy::op::not(policy);
        }
        let encoded = NucTokenBuilder::delegation(vec![policy])
            .audience(Did::nil([0xbb; 33]))
            .subject(Did::nil([0xcc; 33]))
            .command(["nil", "db", "read"])
            .nonce([1, 2, 3])
            .build(&key.into())
            .expect("build failed");
        let err = NucTokenEnvelope::decode(&encoded).expect_err("decode succeeded");
        assert!(matches!(err, NucEnvelopeParseError::Jwt(NucJwtParseError::Json(_, _))));
    }

    #[test]
    fn too_large() {
        let token = " ".repeat(DEFAULT_MAX_RAW_TOKEN_SIZE + 1);
        let err = NucTokenEnvelope::decode(&token).expect_err("decode succeeded");
        assert!(matches!(err, NucEnvelopeParseError::NucTooLarge(_)));
    }
}
