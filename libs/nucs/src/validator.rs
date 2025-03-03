use crate::{
    envelope::{DecodedNucToken, InvalidSignature, NucTokenEnvelope},
    token::{NucToken, ProofHash, TokenBody},
};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use k256::PublicKey;
use std::{
    collections::{HashMap, HashSet},
    fmt, iter,
};

const MAX_CHAIN_LENGTH: usize = 5;

/// Parameters to be used during validation.
pub struct ValidationParameters {
    /// The timestamp to use for token temporal checks.
    pub current_time: DateTime<Utc>,

    /// The maximum allowed chain length.
    pub max_chain_length: usize,

    /// Whether to require the last token in the chain to be an invocation.
    pub require_invocation: bool,
}

impl Default for ValidationParameters {
    fn default() -> Self {
        Self { current_time: Utc::now(), max_chain_length: MAX_CHAIN_LENGTH, require_invocation: true }
    }
}

pub struct NucValidator {
    root_keys: HashSet<Box<[u8]>>,
}

impl NucValidator {
    /// Construct a new NUC validator.
    pub fn new(root_keys: &[PublicKey]) -> Self {
        let root_keys = root_keys.iter().map(|pk| pk.to_sec1_bytes()).collect();
        Self { root_keys }
    }

    /// Validate a NUC.
    pub fn validate(
        &self,
        envelope: NucTokenEnvelope,
        parameters: &ValidationParameters,
    ) -> Result<(), ValidationError> {
        let envelope = envelope.validate_signatures()?;
        let token = &envelope.token().token;
        let proofs = match token.proofs.as_slice() {
            [] if envelope.proofs().is_empty() => {
                return Err(ValidationError::Validation(ValidationKind::NoProofs));
            }
            [] => Vec::new(),
            [hash] => Self::sort_proofs(*hash, envelope.proofs())?,
            _ => return Err(ValidationError::TooManyProofs),
        };

        // Create a sequence [root, ..., token]
        let token_chain = iter::once(token).chain(proofs.iter().copied()).rev();
        Self::validate_proofs(&proofs, parameters, &self.root_keys)?;
        Self::validate_token_chain(token_chain, parameters)?;
        Self::validate_token(token, &proofs, parameters)?;
        Ok(())
    }

    // Validations applied only to the token itself
    fn validate_token(
        token: &NucToken,
        proofs: &[&NucToken],
        parameters: &ValidationParameters,
    ) -> Result<(), ValidationError> {
        match &token.body {
            TokenBody::Delegation(_) => {
                if parameters.require_invocation {
                    return Err(ValidationError::Validation(ValidationKind::NeedInvocation));
                }
            }
            TokenBody::Invocation(_) => {
                let token_json = serde_json::to_value(token).map_err(ValidationError::Serde)?;
                for proof in proofs {
                    Self::validate_policy_matches(proof, &token_json)?;
                }
            }
        };
        Ok(())
    }

    // Validations applied to proofs
    fn validate_proofs(
        proofs: &[&NucToken],
        parameters: &ValidationParameters,
        root_keys: &HashSet<Box<[u8]>>,
    ) -> Result<(), ValidationError> {
        match proofs.last() {
            Some(proof) => {
                if !root_keys.contains(proof.issuer.public_key.as_slice()) {
                    return Err(ValidationError::Validation(ValidationKind::RootKeySignatureMissing));
                }
            }
            None => return Err(ValidationError::Validation(ValidationKind::NoProofs)),
        };

        if proofs.len().saturating_add(1) > parameters.max_chain_length {
            return Err(ValidationError::Validation(ValidationKind::ChainTooLong));
        }

        for proof in proofs {
            match proof.body {
                TokenBody::Delegation(_) => (),
                TokenBody::Invocation(_) => {
                    return Err(ValidationError::Validation(ValidationKind::ProofsMustBeDelegations));
                }
            };
        }
        Ok(())
    }

    // Validations applied to the entire chain (proofs + token).
    fn validate_token_chain<'a, I>(mut tokens: I, parameters: &ValidationParameters) -> Result<(), ValidationError>
    where
        I: Iterator<Item = &'a NucToken> + Clone,
    {
        for (previous, current) in tokens.clone().tuple_windows() {
            Self::validate_relationship_properties(previous, current)?;
        }
        for token in tokens.clone() {
            Self::validate_temporal_properties(token, &parameters.current_time)?;
        }
        if let Some(token) = tokens.nth(1) {
            Self::validate_condition(token.issuer == token.subject, ValidationKind::SubjectNotInChain)?;
        }
        Ok(())
    }

    fn validate_relationship_properties(previous: &NucToken, current: &NucToken) -> Result<(), ValidationError> {
        Self::validate_condition(previous.audience == current.issuer, ValidationKind::IssuerAudienceMismatch)?;
        Self::validate_condition(previous.subject == current.subject, ValidationKind::DifferentSubjects)?;
        Self::validate_condition(
            previous.command.starts_with(&current.command.0),
            ValidationKind::CommandNotAttenuated,
        )?;
        if let Some((previous_not_before, current_not_before)) = previous.not_before.zip(current.not_before) {
            Self::validate_condition(previous_not_before <= current_not_before, ValidationKind::NotBeforeBackwards)?;
        }
        Ok(())
    }

    fn validate_temporal_properties(token: &NucToken, now: &DateTime<Utc>) -> Result<(), ValidationError> {
        Self::validate_condition(token.expires_at.map(|t| now < &t).unwrap_or(true), ValidationKind::TokenExpired)?;
        Self::validate_condition(token.not_before.map(|t| now >= &t).unwrap_or(true), ValidationKind::NotBeforeNotMet)?;
        Ok(())
    }

    fn validate_policy_matches(token: &NucToken, invocation: &serde_json::Value) -> Result<(), ValidationError> {
        match &token.body {
            TokenBody::Delegation(policies) => {
                for policy in policies {
                    if !policy.evaluate(invocation) {
                        return Err(ValidationError::Validation(ValidationKind::PolicyNotMet));
                    }
                }
                Ok(())
            }
            TokenBody::Invocation(_) => Err(ValidationError::Validation(ValidationKind::ProofsMustBeDelegations)),
        }
    }

    // Create a chain of tokens linked by their proof hashes.
    fn sort_proofs(head_hash: ProofHash, proofs: &[DecodedNucToken]) -> Result<Vec<&NucToken>, ValidationError> {
        // Do a first pass and index them all by hash.
        let mut indexed_proofs = HashMap::new();
        for proof in proofs {
            let hash = proof.compute_hash();
            indexed_proofs.insert(hash, &proof.token);
        }

        // Now start from the head hash and walk backwards until we hit the root.
        let mut next_hash = head_hash;
        let mut chain = Vec::new();
        while let Some(proof) = indexed_proofs.remove(&next_hash) {
            chain.push(proof);
            match proof.proofs.as_slice() {
                // We hit the root NUC
                [] => break,
                // Continue with this one
                [hash] => next_hash = *hash,
                _ => return Err(ValidationError::TooManyProofs),
            };
        }
        // Make sure there's nothing left
        if indexed_proofs.is_empty() { Ok(chain) } else { Err(ValidationError::UnchainedProofs(indexed_proofs.len())) }
    }

    fn validate_condition(condition: bool, error_kind: ValidationKind) -> Result<(), ValidationError> {
        if condition { Ok(()) } else { Err(ValidationError::Validation(error_kind)) }
    }
}

/// An error during the validation of a token.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error(transparent)]
    Signature(#[from] InvalidSignature),

    #[error("up to one `prf` in a token is allowed")]
    TooManyProofs,

    #[error("{0} proofs are not part of chain")]
    UnchainedProofs(usize),

    #[error("validation failed: {0}")]
    Validation(ValidationKind),

    #[error("invalid JSON token: {0}")]
    Serde(serde_json::Error),
}

impl From<ValidationKind> for ValidationError {
    fn from(kind: ValidationKind) -> Self {
        Self::Validation(kind)
    }
}

/// A kind of validation that was violated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValidationKind {
    ChainTooLong,
    CommandNotAttenuated,
    DifferentSubjects,
    IssuerAudienceMismatch,
    NeedInvocation,
    NoProofs,
    NotBeforeBackwards,
    NotBeforeNotMet,
    PolicyNotMet,
    ProofsMustBeDelegations,
    RootKeySignatureMissing,
    SubjectNotInChain,
    TokenExpired,
}

impl fmt::Display for ValidationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ValidationKind::*;
        let text = match self {
            ChainTooLong => "token chain is too long",
            CommandNotAttenuated => "command is not an attenuation",
            DifferentSubjects => "different subjects in chain",
            IssuerAudienceMismatch => "issuer/audience mismatch",
            NeedInvocation => "token must be an invocation",
            NoProofs => "need at least one proof",
            NotBeforeBackwards => "`not before` cannot move backwards",
            NotBeforeNotMet => "`not before` date not met",
            PolicyNotMet => "policy not met",
            ProofsMustBeDelegations => "proofs must be delegations",
            RootKeySignatureMissing => "root NUC is not signed by root keypair",
            SubjectNotInChain => "subject not in chain",
            TokenExpired => "token is expired",
        };
        write!(f, "{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::NucTokenBuilder,
        policy::{self},
        token::Did,
    };
    use k256::SecretKey;
    use serde_json::json;
    use std::{ops::Deref, sync::LazyLock};

    static ROOT_SECRET_KEYS: LazyLock<Vec<SecretKey>> =
        LazyLock::new(|| vec![SecretKey::random(&mut rand::thread_rng())]);
    static ROOT_PUBLIC_KEYS: LazyLock<Vec<PublicKey>> =
        LazyLock::new(|| ROOT_SECRET_KEYS.iter().map(|s| s.public_key()).collect());

    // A helper to chain tokens together.
    struct Chainer {
        chain_issuer_audience: bool,
    }

    impl Default for Chainer {
        fn default() -> Self {
            Self { chain_issuer_audience: true }
        }
    }

    impl Chainer {
        fn chain<const N: usize>(&self, mut builders: [Builder; N]) -> NucTokenEnvelope {
            if self.chain_issuer_audience {
                // Chain the issuer -> audience in every token
                for i in 0..builders.len().saturating_sub(1) {
                    let next = &builders[i + 1];
                    let issuer_key: [u8; 33] = next.owner_key.public_key().to_sec1_bytes().deref().try_into().unwrap();

                    let previous = &mut builders[i];
                    previous.builder = previous.builder.clone().audience(Did::nil(issuer_key));
                }
            }

            // Now chain the proof hashes
            let mut builders = builders.into_iter();
            let token = builders.next().expect("no builders").build();
            let mut envelope = NucTokenEnvelope::decode(&token).unwrap();
            for mut builder in builders {
                let next = envelope.validate_signatures().expect("signature validation failed");
                builder.builder = builder.builder.proof(next);

                let token = builder.build();
                envelope = NucTokenEnvelope::decode(&token).unwrap();
            }
            envelope
        }
    }

    struct Asserter {
        parameters: ValidationParameters,
    }

    impl Asserter {
        fn new(parameters: ValidationParameters) -> Self {
            Self { parameters }
        }

        fn log_tokens(envelope: &NucTokenEnvelope) {
            // Log this so we can debug tests based on their output
            println!("Token being asserted: {}", serde_json::to_string_pretty(envelope.token().token()).unwrap());
            println!(
                "Proofs for it: {}",
                serde_json::to_string_pretty(&envelope.proofs().iter().map(|d| &d.token).collect::<Vec<_>>()).unwrap()
            );
        }

        fn assert_failure<E: Into<ValidationError>>(&self, envelope: NucTokenEnvelope, expected_failure: E) {
            Self::log_tokens(&envelope);

            let expected_failure = expected_failure.into();
            match NucValidator::new(&ROOT_PUBLIC_KEYS).validate(envelope, &self.parameters) {
                Ok(()) => panic!("validation succeeded"),
                Err(e) if e.to_string() == expected_failure.to_string() => (),
                Err(e) => panic!("unexpected type of failure: {e}"),
            };
        }

        fn assert_success(&self, envelope: NucTokenEnvelope) {
            Self::log_tokens(&envelope);
            NucValidator::new(&ROOT_PUBLIC_KEYS).validate(envelope, &self.parameters).expect("validation failed");
        }
    }

    impl Default for Asserter {
        fn default() -> Self {
            let parameters = ValidationParameters { require_invocation: false, ..Default::default() };
            Self { parameters }
        }
    }

    struct Builder {
        builder: NucTokenBuilder,
        owner_key: SecretKey,
    }

    impl Builder {
        fn build(self) -> String {
            self.builder.build(&self.owner_key.into()).expect("failed to build")
        }
    }

    trait NucTokenBuilderExt: Sized {
        fn issued_by_root(self) -> Builder;
        fn issued_by(self, owner_key: SecretKey) -> Builder;
    }

    impl NucTokenBuilderExt for NucTokenBuilder {
        fn issued_by_root(self) -> Builder {
            Builder { builder: self, owner_key: ROOT_SECRET_KEYS[0].clone() }
        }

        fn issued_by(self, owner_key: SecretKey) -> Builder {
            Builder { builder: self, owner_key }
        }
    }

    trait DidExt: Sized {
        fn from_secret_key(secret_key: &SecretKey) -> Self;
    }

    impl DidExt for Did {
        fn from_secret_key(secret_key: &SecretKey) -> Self {
            let public_key: [u8; 33] = secret_key.public_key().to_sec1_bytes().deref().try_into().unwrap();
            Did::nil(public_key)
        }
    }

    fn secret_key() -> SecretKey {
        SecretKey::random(&mut rand::thread_rng())
    }

    // Create a delegation with the most common fields already set so we don't need to deal
    // with them in every single test.
    fn delegation(subject: &SecretKey) -> NucTokenBuilder {
        NucTokenBuilder::delegation([]).audience(Did::nil([0xde; 33])).subject(Did::from_secret_key(subject)).nonce([1])
    }

    #[test]
    fn unlinked_chain() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        // Chain 2.
        let envelope = Chainer::default().chain([base.clone().issued_by_root(), base.clone().issued_by(key)]).encode();

        // Now chain an extra one that nobody refers to.
        let last = base.clone().issued_by_root().build();
        let token = format!("{envelope}/{last}");
        let envelope = NucTokenEnvelope::decode(&token).expect("decode failed");
        Asserter::default().assert_failure(envelope, ValidationError::UnchainedProofs(1));
    }

    #[test]
    fn chain_too_long() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let last = base.clone();
        let envelope = Chainer::default().chain([
            base.clone().issued_by_root(),
            base.clone().issued_by(key.clone()),
            last.issued_by(key.clone()),
        ]);
        let parameters = ValidationParameters { max_chain_length: 2, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::ChainTooLong);
    }

    #[test]
    fn command_not_attenuated() {
        let key = secret_key();
        let base = delegation(&key);
        let root = base.clone().command(["nil"]).issued_by_root();
        let last = base.command(["bar"]).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::CommandNotAttenuated);
    }

    #[test]
    fn different_subjects() {
        let key1 = secret_key();
        let key2 = secret_key();
        let root = delegation(&key1).command(["nil"]).issued_by_root();
        let last = delegation(&key2).command(["nil"]).issued_by(key2);
        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::DifferentSubjects);
    }

    #[test]
    fn issuer_audience_mismatch() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().audience(Did::nil([0xaa; 33])).issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer { chain_issuer_audience: false }.chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::IssuerAudienceMismatch);
    }

    #[test]
    fn need_invocation() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { require_invocation: true, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NeedInvocation);
    }

    #[test]
    fn no_proofs() {
        let key = secret_key();
        let token = delegation(&key).command(["nil"]).issued_by_root().build();
        let envelope = NucTokenEnvelope::decode(&token).expect("invalid JWT");
        Asserter::default().assert_failure(envelope, ValidationKind::NoProofs);
    }

    #[test]
    fn not_before_backwards() {
        let now = DateTime::from_timestamp(10, 0).unwrap();
        let root_not_before = DateTime::from_timestamp(5, 0).unwrap();
        let last_not_before = DateTime::from_timestamp(3, 0).unwrap();

        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().not_before(root_not_before).issued_by_root();
        let last = base.not_before(last_not_before).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { current_time: now, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NotBeforeBackwards);
    }

    #[test]
    fn proof_not_before_not_met() {
        let now = DateTime::from_timestamp(0, 0).unwrap();
        let not_before = DateTime::from_timestamp(10, 0).unwrap();

        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().not_before(not_before).issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { current_time: now, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NotBeforeNotMet);
    }

    #[test]
    fn last_not_before_not_met() {
        let now = DateTime::from_timestamp(0, 0).unwrap();
        let not_before = DateTime::from_timestamp(10, 0).unwrap();

        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.not_before(not_before).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { current_time: now, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NotBeforeNotMet);
    }

    #[test]
    fn root_policy_not_met() {
        let key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .nonce([1])
            .issued_by_root();
        let invocation = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject)
            .audience(Did::nil([0xaa; 33]))
            .command(["nil"])
            .nonce([1])
            .issued_by(key);

        let envelope = Chainer::default().chain([root, invocation]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyNotMet);
    }

    #[test]
    fn last_policy_not_met() {
        let subject_key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&subject_key);
        let root =
            NucTokenBuilder::delegation([]).subject(subject.clone()).command(["nil"]).nonce([1]).issued_by_root();
        let intermediate = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .nonce([1])
            .issued_by(subject_key);
        let invocation = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject)
            .audience(Did::nil([0xaa; 33]))
            .command(["nil"])
            .nonce([1])
            .issued_by(secret_key());

        let envelope = Chainer::default().chain([root, intermediate, invocation]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyNotMet);
    }

    #[test]
    fn proofs_must_be_delegations() {
        let key = secret_key();
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject.clone())
            .command(["nil"])
            .nonce([1])
            .issued_by_root();
        let last = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject)
            .audience(Did::nil([0xaa; 33]))
            .command(["nil"])
            .nonce([1])
            .issued_by(key);

        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::ProofsMustBeDelegations);
    }

    #[test]
    fn root_key_signature_missing() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by(key.clone());
        let last = base.audience(Did::nil([0xaa; 33])).issued_by(key);

        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::RootKeySignatureMissing);
    }

    #[test]
    fn subject_not_in_chain() {
        let subject_key = secret_key();
        let key = secret_key();
        let base = delegation(&subject_key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.audience(Did::nil([0xaa; 33])).issued_by(key);

        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::SubjectNotInChain);
    }

    #[test]
    fn root_token_expired() {
        let now = DateTime::from_timestamp(10, 0).unwrap();
        let expires_at = DateTime::from_timestamp(5, 0).unwrap();

        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().expires_at(expires_at).issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { current_time: now, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::TokenExpired);
    }

    #[test]
    fn last_token_expired() {
        let now = DateTime::from_timestamp(10, 0).unwrap();
        let expires_at = DateTime::from_timestamp(5, 0).unwrap();

        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.expires_at(expires_at).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters { current_time: now, ..Default::default() };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::TokenExpired);
    }

    #[test]
    fn valid_token() {
        let subject_key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&subject_key);
        let root = NucTokenBuilder::delegation([policy::op::eq(".args.foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .nonce([1])
            .issued_by_root();
        let intermediate = NucTokenBuilder::delegation([policy::op::eq(".args.bar", json!(1337))])
            .subject(subject.clone())
            .command(["nil", "bar"])
            .nonce([1])
            .issued_by(subject_key);
        let invocation = NucTokenBuilder::invocation(json!({"foo": 42, "bar": 1337}).as_object().cloned().unwrap())
            .subject(subject)
            .audience(Did::nil([0xaa; 33]))
            .command(["nil", "bar", "foo"])
            .nonce([1])
            .issued_by(secret_key());

        let envelope = Chainer::default().chain([root, intermediate, invocation]);
        let parameters = ValidationParameters { require_invocation: true, ..Default::default() };
        Asserter::new(parameters).assert_success(envelope);
    }
}
