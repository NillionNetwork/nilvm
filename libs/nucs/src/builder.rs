use crate::{
    envelope::{DecodedNucToken, JwtAlgorithm, JwtHeader, NucTokenEnvelope, SignaturesValidated},
    policy::Policy,
    token::{Command, Did, JsonObject, NucToken, TokenBody},
};
use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use k256::ecdsa::{Signature, SigningKey, VerifyingKey};
use serde::Serialize;
use signature::Signer;
use std::{ops::Deref, time::Duration};

// Helper to simplify unwrapping options in the builder
macro_rules! try_get {
    ($option:ident) => {
        $option.ok_or(NucTokenBuildError::MissingField(stringify!($option)))
    };
}

/// A NUC token builder.
#[derive(Clone, Debug)]
pub struct NucTokenBuilder {
    body: TokenBody,
    audience: Option<Did>,
    subject: Option<Did>,
    not_before: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    command: Option<Command>,
    meta: Option<JsonObject>,
    nonce: Vec<u8>,
    proof: Option<NucTokenEnvelope<SignaturesValidated>>,
}

impl NucTokenBuilder {
    /// Construct a new builder.
    pub fn new(body: TokenBody) -> Self {
        Self {
            body,
            audience: Default::default(),
            subject: Default::default(),
            not_before: Default::default(),
            expires_at: Default::default(),
            command: Default::default(),
            meta: Default::default(),
            nonce: Default::default(),
            proof: Default::default(),
        }
    }

    /// Make this a delegation token using the given policy.
    pub fn delegation<I>(policies: I) -> Self
    where
        I: Into<Vec<Policy>>,
    {
        Self::new(TokenBody::Delegation(policies.into()))
    }

    /// Make this an invocation token using the given policy.
    pub fn invocation(args: JsonObject) -> Self {
        Self::new(TokenBody::Invocation(args))
    }

    /// Create a new builder using the given envelope as a base.
    ///
    /// This will pull as many parameters as it can from the given token, such as:
    ///
    /// * The subject.
    /// * The command.
    /// * Using it as a proof.
    pub fn extending(envelope: NucTokenEnvelope<SignaturesValidated>) -> Result<Self, ExtendTokenError> {
        let token = &envelope.token().token;
        if let TokenBody::Invocation(_) = &token.body {
            return Err(ExtendTokenError);
        }
        let output = Self::new(TokenBody::Delegation(vec![]))
            .command(token.command.clone())
            .subject(token.subject.clone())
            .proof(envelope);
        Ok(output)
    }

    /// Set the audience for this token.
    pub fn audience(mut self, did: Did) -> Self {
        self.audience = Some(did);
        self
    }

    /// Set the subject for this token.
    pub fn subject(mut self, did: Did) -> Self {
        self.subject = Some(did);
        self
    }

    /// Set the expiration time for this token.
    pub fn expires_at(mut self, timestamp: DateTime<Utc>) -> Self {
        self.expires_at = Some(timestamp);
        self
    }

    /// Set the expiration time for this token based on an offset from the current time.
    pub fn expires_in(mut self, offset: Duration) -> Self {
        self.expires_at = Some(Utc::now() + offset);
        self
    }

    /// Set the timestamp when this token first becomes valid for this token.
    pub fn not_before(mut self, timestamp: DateTime<Utc>) -> Self {
        self.not_before = Some(timestamp);
        self
    }

    /// Set the command for this token.
    pub fn command<T: Into<Command>>(mut self, command: T) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Set the metadata for this token.
    pub fn meta(mut self, meta: JsonObject) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Set the nonce for this token.
    pub fn nonce<T: Into<Vec<u8>>>(mut self, nonce: T) -> Self {
        self.nonce = nonce.into();
        self
    }

    /// Use the given envelope as a proof to be chained into the built token.
    pub fn proof(mut self, envelope: NucTokenEnvelope<SignaturesValidated>) -> Self {
        self.proof = Some(envelope);
        self
    }

    /// Set the body.
    pub fn body(mut self, body: TokenBody) -> Self {
        self.body = body;
        self
    }

    /// Build a NUC token.
    pub fn build(self, issuer_key: &SigningKey) -> Result<String, NucTokenBuildError> {
        let Self { body, audience, subject, not_before, expires_at, command, meta, nonce, proof } = self;
        let audience = try_get!(audience)?;
        let subject = try_get!(subject)?;
        let command = try_get!(command)?;

        // Default to a 16 byte nonce if none is set.
        let nonce = match nonce.is_empty() {
            true => rand::random::<[u8; 16]>().to_vec(),
            false => nonce,
        };

        let public_key = VerifyingKey::from(issuer_key);
        let public_key =
            public_key.to_sec1_bytes().deref().try_into().map_err(|_| NucTokenBuildError::IssuerPublicKey)?;
        let issuer = Did::new(public_key);
        let mut token =
            NucToken { issuer, audience, subject, not_before, expires_at, command, body, meta, nonce, proofs: vec![] };

        let mut all_proofs = Vec::new();
        if let Some(envelope) = proof {
            let hash = envelope.token().compute_hash();
            let (root_proof, proofs) = envelope.into_parts();
            token.proofs.push(hash);

            all_proofs.push(root_proof);
            all_proofs.extend(proofs);
        };
        Self::serialize(token, issuer_key, all_proofs)
    }

    fn serialize(
        token: NucToken,
        issuer_key: &SigningKey,
        proofs: Vec<DecodedNucToken>,
    ) -> Result<String, NucTokenBuildError> {
        use NucTokenBuildError::*;
        let header = JwtHeader { algorithm: JwtAlgorithm::Es256k };
        let header = to_base64_json(&header).map_err(|e| EncodingHeader(e.to_string()))?;
        let token = to_base64_json(&token).map_err(|e| EncodingToken(e.to_string()))?;
        let mut output = format!("{header}.{token}");
        let signature: Signature = issuer_key.sign(output.as_bytes());
        output.push('.');
        output.push_str(&to_base64(signature.to_bytes()));
        for proof in proofs {
            output.push('/');
            output.push_str(&proof.to_jwt());
        }
        Ok(output)
    }
}

/// An error when exending a token from another.
#[derive(Debug, thiserror::Error)]
#[error("invocations cannot be extended")]
pub struct ExtendTokenError;

/// An error when constructing a token.
#[derive(Debug, thiserror::Error)]
pub enum NucTokenBuildError {
    #[error("required field missing: {0}")]
    MissingField(&'static str),

    #[error("encoding header: {0}")]
    EncodingHeader(String),

    #[error("encoding token: {0}")]
    EncodingToken(String),

    #[error("proof serializing: {0}")]
    ProofSerializing(serde_json::Error),

    #[error("invalid issuer public key")]
    IssuerPublicKey,
}

pub(crate) fn to_base64<T: AsRef<[u8]>>(input: T) -> String {
    BASE64_URL_SAFE_NO_PAD.encode(input)
}

pub(crate) fn to_base64_json<T: Serialize>(input: &T) -> Result<String, serde_json::Error> {
    let input = serde_json::to_vec(input)?;
    Ok(to_base64(&input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        envelope::{from_base64, NucTokenEnvelope},
        policy,
    };
    use k256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
    use serde::de::DeserializeOwned;
    use serde_json::json;

    fn from_base64_json<T: DeserializeOwned>(input: &str) -> T {
        let input = from_base64(input).expect("invalid base 64");
        serde_json::from_slice(&input).expect("invalid JSON")
    }

    #[test]
    fn minimal_token() {
        let key = SecretKey::random(&mut rand::thread_rng());
        NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .build(&key.into())
            .expect("build failed");
    }

    #[test]
    fn extend_token() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let base = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .build(&key.clone().into())
            .expect("build failed");
        let base = NucTokenEnvelope::decode(&base).expect("decode failed").validate_signatures().unwrap();
        let next = NucTokenBuilder::extending(base.clone())
            .expect("extending failed")
            .audience(Did::new([0xdd; 33]))
            .build(&key.into())
            .expect("build failed");
        let next = NucTokenEnvelope::decode(&next).expect("decode failed").validate_signatures().unwrap();
        let (token, proofs) = next.into_parts();
        let token = token.token;
        assert_eq!(token.command, base.token().token.command);
        assert_eq!(token.subject, base.token().token.subject);
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].token, base.token().token);
    }

    #[test]
    fn encode_decode() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let token = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .not_before(DateTime::from_timestamp(1740494955, 0).unwrap())
            .expires_at(DateTime::from_timestamp(1740495955, 0).unwrap())
            .nonce([1, 2, 3])
            .meta(json!({"name": "bob"}).as_object().cloned().unwrap())
            .build(&(&key).into())
            .expect("failed to build");
        let mut token = token.split('.');
        let header = token.next().expect("no header");
        let header = from_base64(header).unwrap();

        let header: serde_json::Value = serde_json::from_slice(&header).expect("invalid header");
        assert_eq!(header, json!({"alg": "ES256K"}));

        let nuc = token.next().expect("no token");
        let nuc: NucToken = from_base64_json(nuc);
        let issuer: [u8; 33] = key.public_key().to_encoded_point(true).as_bytes().try_into().unwrap();
        let issuer = Did::new(issuer);
        let expected = NucToken {
            issuer,
            audience: Did::new([0xbb; 33]),
            subject: Did::new([0xcc; 33]),
            not_before: Some(DateTime::from_timestamp(1740494955, 0).unwrap()),
            expires_at: Some(DateTime::from_timestamp(1740495955, 0).unwrap()),
            command: ["nil", "db", "read"].into(),
            body: TokenBody::Delegation(vec![policy::op::eq(".foo", json!(42))]),
            proofs: vec![],
            nonce: vec![1, 2, 3],
            meta: Some(json!({ "name": "bob" }).as_object().cloned().unwrap()),
        };
        assert_eq!(nuc, expected);
    }

    #[test]
    fn chain() {
        // Build a root NUC
        let root_key = SecretKey::random(&mut rand::thread_rng());
        let root = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .build(&root_key.into())
            .expect("build failed");
        let root = NucTokenEnvelope::decode(&root)
            .expect("decoding failed")
            .validate_signatures()
            .expect("signaturte validation failed");

        // Build a delegation using the above proof
        let other_key = SecretKey::random(&mut rand::thread_rng());
        let delegation = NucTokenBuilder::delegation(vec![policy::op::eq(".foo", json!(42))])
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .proof(root.clone())
            .build(&other_key.into())
            .expect("build failed");
        let delegation = NucTokenEnvelope::decode(&delegation)
            .expect("decoding failed")
            .validate_signatures()
            .expect("signature validation failed");

        // Ensure the tokens are linked.
        assert_eq!(delegation.token().token().proofs, vec![root.token().compute_hash()]);
        assert_eq!(delegation.proofs().len(), 1);
        assert_eq!(&delegation.proofs()[0].token, root.token().token());

        // Build an invocation using the above as proof.
        let yet_another_key = SecretKey::random(&mut rand::thread_rng());
        let invocation = NucTokenBuilder::invocation(json!({"foo": 42}).as_object().cloned().unwrap())
            .audience(Did::new([0xbb; 33]))
            .subject(Did::new([0xcc; 33]))
            .command(["nil", "db", "read"])
            .proof(delegation.clone())
            .build(&yet_another_key.into())
            .expect("build failed");

        let invocation = NucTokenEnvelope::decode(&invocation)
            .expect("decoding failed")
            .validate_signatures()
            .expect("signature validation failed");

        // Ensure both delegations are included as proofs
        assert_eq!(invocation.token().token().proofs, vec![delegation.token().compute_hash()]);
        assert_eq!(invocation.proofs().len(), 2);
        assert_eq!(&invocation.proofs()[0].token, delegation.token().token());
        assert_eq!(&invocation.proofs()[1].token, root.token().token());
    }

    #[test]
    fn decode_specific() {
        // this is a specific token generated by the above function
        let token = "eyJhbGciOiJFUzI1NksifQ.eyJpc3MiOiJkaWQ6bmlsOjAyMjZhNGQ0YTRhNWZhZGUxMmM1ZmYwZWM5YzQ3MjQ5ZjIxY2Y3N2EyMDI3NTFmOTU5ZDVjNzc4ZjBiNjUyYjcxNiIsImF1ZCI6ImRpZDpuaWw6YmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwic3ViIjoiZGlkOm5pbDpjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2MiLCJjbWQiOiIvbmlsL2RiL3JlYWQiLCJhcmdzIjp7ImZvbyI6NDJ9LCJub25jZSI6IjAxMDIwMyIsInByZiI6WyJjOTA0YzVhMWFiMzY5YWVhMWI0ZDlkMTkwMmE0NmU2ZWY5NGFhYjk2OTY0YmI1MWQ2MWE2MWIwM2UyM2Q1ZGZmIl19.ufDYxqoSVNVETrVKReu0h_Piul5c6RoC_VnGGLw04mkyn2OMrtQjK92sGXNHCjlp7T9prIwxX14ZB_N3gx7hPg/eyJhbGciOiJFUzI1NksifQ.eyJpc3MiOiJkaWQ6bmlsOjAzNmY3MDdmYmVmMGI3NTIxMzgwOGJiYmY1NGIxODIxNzZmNTMyMGZhNTIwY2I4MTlmMzViNWJhZjIzMjM4YTAxNSIsImF1ZCI6ImRpZDpuaWw6YmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwic3ViIjoiZGlkOm5pbDpjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2MiLCJjbWQiOiIvbmlsL2RiL3JlYWQiLCJwb2wiOltbIj09IiwiLmZvbyIsNDJdXSwibm9uY2UiOiIwMTAyMDMiLCJwcmYiOlsiODZjZGI1ZjZjN2M3NDFkMDBmNmI4ODMzZDI0ZjdlY2Y5MWFjOGViYzI2MzA3MmZkYmU0YTZkOTQ5NzIwMmNiNCJdfQ.drGzkA0hYP8h62GxNN3fhi9bKjYgjpSy4cM52-9RsyB7JD6O6K1wRsg_x1hv8ladPmChpwDVVXOzjNr2NRVntA/eyJhbGciOiJFUzI1NksifQ.eyJpc3MiOiJkaWQ6bmlsOjAzOTU5MGNjYWYxMDI0ZjQ5YzljZjc0M2Y4YTZlZDQyMDNlNzgyZThlZTA5YWZhNTNkMWI1NzY0OTg0NjEyMzQyNSIsImF1ZCI6ImRpZDpuaWw6YmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiYmJiIiwic3ViIjoiZGlkOm5pbDpjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2MiLCJjbWQiOiIvbmlsL2RiL3JlYWQiLCJwb2wiOltbIj09IiwiLmZvbyIsNDJdXSwibm9uY2UiOiIwMTAyMDMiLCJwcmYiOltdfQ.o3lnQxCjDCW10UuRABrHp8FpB_C6q1xgEGvfuXTb7Epp63ry8R2h0wHjToDKDFmkmUmO2jcBkrttuy8kftV6og";
        NucTokenEnvelope::decode(token).expect("failed to decode");
    }
}
