use crate::{
    envelope::{DecodedNucToken, NucTokenEnvelope},
    policy::{ConnectorPolicy, Operator, OperatorPolicy, Policy},
    token::{Did, NucToken, ProofHash, TokenBody},
};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use k256::PublicKey;
use std::{
    collections::{HashMap, HashSet},
    fmt, iter,
    ops::Deref,
};

const MAX_CHAIN_LENGTH: usize = 5;
const MAX_POLICY_WIDTH: usize = 10;
const MAX_POLICY_DEPTH: usize = 5;
const REVOCATION_COMMAND: &[&str] = &["nuc", "revoke"];

/// The result of validating a NUC token.
pub type ValidationResult = Result<(), ValidationError>;

/// Parameters to be used during validation.
#[derive(Debug)]
pub struct ValidationParameters {
    /// The timestamp to use for token temporal checks.
    pub current_time: DateTime<Utc>,

    /// The maximum allowed chain length.
    pub max_chain_length: usize,

    /// The maximum width of a policy.
    pub max_policy_width: usize,

    /// The maximum depth of a policy.
    pub max_policy_depth: usize,

    /// The requirements for the token being validated.
    pub token_requirements: TokenTypeRequirements,
}

impl Default for ValidationParameters {
    fn default() -> Self {
        Self {
            current_time: Utc::now(),
            max_chain_length: MAX_CHAIN_LENGTH,
            max_policy_width: MAX_POLICY_WIDTH,
            max_policy_depth: MAX_POLICY_DEPTH,
            token_requirements: Default::default(),
        }
    }
}

/// The requirements for the token being validated.
#[derive(Debug, Default)]
pub enum TokenTypeRequirements {
    /// Require an invocation for the given DID.
    Invocation(Did),

    /// Require a delegation for the given DID.
    Delegation(Did),

    /// Apply no token type requirements, meaning we're okay with any invocation and/or delegation.
    #[default]
    None,
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
        parameters: ValidationParameters,
    ) -> Result<ValidatedNucToken, ValidationError> {
        // Perform this one check before validating signatures to avoid doing contly work
        if envelope.proofs().len().saturating_add(1) > parameters.max_chain_length {
            return Err(ValidationError::Validation(ValidationKind::ChainTooLong));
        }

        let token = &envelope.token().token;
        let proofs = match token.proofs.as_slice() {
            [] => Vec::new(),
            [hash] => Self::sort_proofs(*hash, envelope.proofs())?,
            _ => return Err(ValidationKind::TooManyProofs.into()),
        };

        // Create a sequence [root, ..., token]
        let token_chain = iter::once(token).chain(proofs.iter().copied()).rev();
        Self::validate_proofs(&proofs, &self.root_keys)?;
        Self::validate_token_chain(token_chain, &parameters)?;
        Self::validate_token(token, &proofs, &parameters.token_requirements)?;

        // Signature validation is done at the end as it's arguably the most expensive part of the
        // validation process.
        let envelope = envelope.validate_signatures().map_err(|_| ValidationKind::InvalidSignatures)?;
        let (token, proofs) = envelope.into_parts();
        let validated_token =
            ValidatedNucToken { token: token.token, proofs: proofs.into_iter().map(|proof| proof.token).collect() };
        Ok(validated_token)
    }

    // Validations applied only to the token itself
    fn validate_token(
        token: &NucToken,
        proofs: &[&NucToken],
        requirements: &TokenTypeRequirements,
    ) -> ValidationResult {
        match &token.body {
            TokenBody::Delegation(_) => {
                match requirements {
                    TokenTypeRequirements::Invocation(_) => {
                        return Err(ValidationError::Validation(ValidationKind::NeedInvocation));
                    }
                    TokenTypeRequirements::Delegation(did) => {
                        Self::validate_condition(&token.audience == did, ValidationKind::InvalidAudience)?
                    }
                    TokenTypeRequirements::None => (),
                };
            }
            TokenBody::Invocation(_) => {
                match requirements {
                    TokenTypeRequirements::Invocation(did) => {
                        Self::validate_condition(&token.audience == did, ValidationKind::InvalidAudience)?;
                    }
                    TokenTypeRequirements::Delegation(_) => {
                        return Err(ValidationError::Validation(ValidationKind::NeedDelegation));
                    }
                    TokenTypeRequirements::None => (),
                }
                let token_json = serde_json::to_value(token).map_err(ValidationError::Serde)?;
                for proof in proofs {
                    Self::validate_policy_matches(proof, &token_json)?;
                }
            }
        };
        Ok(())
    }

    // Validations applied to proofs
    fn validate_proofs(proofs: &[&NucToken], root_keys: &HashSet<Box<[u8]>>) -> ValidationResult {
        let contains_root_signature = match proofs.last() {
            Some(proof) => root_keys.contains(proof.issuer.public_key.as_slice()),
            // if there's no proof but also no root keys we don't need one.
            None => root_keys.is_empty(),
        };
        if !contains_root_signature {
            return Err(ValidationError::Validation(ValidationKind::RootKeySignatureMissing));
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
        for token in tokens.clone() {
            if let TokenBody::Delegation(policies) = &token.body {
                Self::validate_policies_properties(policies, parameters)?;
            }
        }
        if let Some(token) = tokens.nth(1) {
            Self::validate_condition(token.issuer == token.subject, ValidationKind::SubjectNotInChain)?;
        }
        Ok(())
    }

    fn validate_policies_properties(policies: &[Policy], parameters: &ValidationParameters) -> ValidationResult {
        Self::validate_condition(policies.len() <= parameters.max_policy_width, ValidationKind::PolicyTooWide)?;
        for policy in policies {
            let properties = PolicyTreeProperties::from(policy);
            Self::validate_condition(
                properties.max_policy_width <= parameters.max_policy_width,
                ValidationKind::PolicyTooWide,
            )?;
            Self::validate_condition(
                properties.max_depth <= parameters.max_policy_depth,
                ValidationKind::PolicyTooDeep,
            )?;
        }
        Ok(())
    }

    fn validate_relationship_properties(previous: &NucToken, current: &NucToken) -> ValidationResult {
        Self::validate_condition(previous.audience == current.issuer, ValidationKind::IssuerAudienceMismatch)?;
        Self::validate_condition(previous.subject == current.subject, ValidationKind::DifferentSubjects)?;
        Self::validate_condition(
            current.command.is_attenuation_of(&previous.command) || current.command.0 == REVOCATION_COMMAND,
            ValidationKind::CommandNotAttenuated,
        )?;
        if let Some((previous_not_before, current_not_before)) = previous.not_before.zip(current.not_before) {
            Self::validate_condition(previous_not_before <= current_not_before, ValidationKind::NotBeforeBackwards)?;
        }
        Ok(())
    }

    fn validate_temporal_properties(token: &NucToken, now: &DateTime<Utc>) -> ValidationResult {
        Self::validate_condition(token.expires_at.map(|t| now < &t).unwrap_or(true), ValidationKind::TokenExpired)?;
        Self::validate_condition(token.not_before.map(|t| now >= &t).unwrap_or(true), ValidationKind::NotBeforeNotMet)?;
        Ok(())
    }

    fn validate_policy_matches(token: &NucToken, invocation: &serde_json::Value) -> ValidationResult {
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
        loop {
            let proof = indexed_proofs.remove(&next_hash).ok_or(ValidationKind::MissingProof)?;
            chain.push(proof);
            match proof.proofs.as_slice() {
                // We hit the root NUC
                [] => break,
                // Continue with this one
                [hash] => next_hash = *hash,
                _ => return Err(ValidationKind::TooManyProofs.into()),
            };
        }
        // Make sure there's nothing left
        if indexed_proofs.is_empty() { Ok(chain) } else { Err(ValidationKind::UnchainedProofs.into()) }
    }

    fn validate_condition(condition: bool, error_kind: ValidationKind) -> Result<(), ValidationError> {
        if condition { Ok(()) } else { Err(ValidationError::Validation(error_kind)) }
    }
}

/// A validated NUC token along with its proofs.
#[derive(Debug)]
pub struct ValidatedNucToken {
    /// The token.
    pub token: NucToken,

    /// The proofs for the token.
    ///
    /// These are sorted in the way the chain was built, starting from `token`'s proof. That is:
    ///
    /// ```no_compile
    /// token proof -> proofs[0] -> ... -> root_token.
    /// ```
    pub proofs: Vec<NucToken>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PolicyTreeProperties {
    max_depth: usize,
    max_policy_width: usize,
}

impl From<&Policy> for PolicyTreeProperties {
    fn from(policy: &Policy) -> Self {
        match policy {
            Policy::Operator(policy) => Self::from(policy),
            Policy::Connector(policy) => Self::from(policy),
        }
    }
}

impl From<&OperatorPolicy> for PolicyTreeProperties {
    fn from(policy: &OperatorPolicy) -> Self {
        use Operator::*;
        match &policy.operator {
            Equals(_) | NotEquals(_) => Self { max_depth: 1, max_policy_width: 1 },
            AnyOf(values) => Self { max_depth: 1, max_policy_width: values.len() },
        }
    }
}

impl From<&ConnectorPolicy> for PolicyTreeProperties {
    fn from(policy: &ConnectorPolicy) -> Self {
        use ConnectorPolicy::*;
        let mut output = match policy {
            And(policies) | Or(policies) => {
                let mut output = PolicyTreeProperties { max_depth: 0, max_policy_width: policies.len() };
                for policy in policies {
                    let properties = Self::from(policy);
                    output.max_depth = output.max_depth.max(properties.max_depth);
                    output.max_policy_width = output.max_policy_width.max(properties.max_policy_width);
                }
                output
            }
            Not(policy) => Self::from(policy.deref()),
        };
        output.max_depth += 1;
        output
    }
}

/// An error during the validation of a token.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
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
    InvalidAudience,
    InvalidSignatures,
    IssuerAudienceMismatch,
    MissingProof,
    NeedDelegation,
    NeedInvocation,
    NotBeforeBackwards,
    NotBeforeNotMet,
    PolicyNotMet,
    PolicyTooDeep,
    PolicyTooWide,
    ProofsMustBeDelegations,
    RootKeySignatureMissing,
    SubjectNotInChain,
    TokenExpired,
    TooManyProofs,
    UnchainedProofs,
}

impl fmt::Display for ValidationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ValidationKind::*;
        let text = match self {
            ChainTooLong => "token chain is too long",
            CommandNotAttenuated => "command is not an attenuation",
            DifferentSubjects => "different subjects in chain",
            InvalidAudience => "invalid audience",
            InvalidSignatures => "invalid signature",
            IssuerAudienceMismatch => "issuer/audience mismatch",
            MissingProof => "proof is missing",
            NeedDelegation => "token must be a delegation",
            NeedInvocation => "token must be an invocation",
            NotBeforeBackwards => "`not before` cannot move backwards",
            NotBeforeNotMet => "`not before` date not met",
            PolicyNotMet => "policy not met",
            PolicyTooDeep => "policy is too deep",
            PolicyTooWide => "policy is too wide",
            ProofsMustBeDelegations => "proofs must be delegations",
            RootKeySignatureMissing => "root NUC is not signed by root keypair",
            SubjectNotInChain => "subject not in chain",
            TokenExpired => "token is expired",
            TooManyProofs => "up to one `prf` in a token is allowed",
            UnchainedProofs => "extra proofs not part of chain provided",
        };
        write!(f, "{text}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::{to_base64, NucTokenBuilder},
        envelope::from_base64,
        policy::{self},
        token::Did,
    };
    use k256::SecretKey;
    use rstest::rstest;
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
                    previous.builder = previous.builder.clone().audience(Did::new(issuer_key));
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
        root_keys: Vec<PublicKey>,
    }

    impl Asserter {
        fn new(parameters: ValidationParameters) -> Self {
            Self { parameters, root_keys: ROOT_PUBLIC_KEYS.clone() }
        }

        fn log_tokens(envelope: &NucTokenEnvelope) {
            // Log this so we can debug tests based on their output
            println!("Token being asserted: {}", serde_json::to_string_pretty(envelope.token().token()).unwrap());
            println!(
                "Proofs for it: {}",
                serde_json::to_string_pretty(&envelope.proofs().iter().map(|d| &d.token).collect::<Vec<_>>()).unwrap()
            );
        }

        fn assert_failure<E: Into<ValidationError>>(self, envelope: NucTokenEnvelope, expected_failure: E) {
            Self::log_tokens(&envelope);

            let expected_failure = expected_failure.into();
            match NucValidator::new(&self.root_keys).validate(envelope, self.parameters) {
                Ok(_) => panic!("validation succeeded"),
                Err(e) if e.to_string() == expected_failure.to_string() => (),
                Err(e) => panic!("unexpected type of failure: {e}"),
            };
        }

        fn assert_success(self, envelope: NucTokenEnvelope) -> ValidatedNucToken {
            Self::log_tokens(&envelope);
            NucValidator::new(&self.root_keys).validate(envelope, self.parameters).expect("validation failed")
        }
    }

    impl Default for Asserter {
        fn default() -> Self {
            let parameters = ValidationParameters::default();
            Self::new(parameters)
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
            Did::new(public_key)
        }
    }

    fn secret_key() -> SecretKey {
        SecretKey::random(&mut rand::thread_rng())
    }

    // Create a delegation with the most common fields already set so we don't need to deal
    // with them in every single test.
    fn delegation(subject: &SecretKey) -> NucTokenBuilder {
        NucTokenBuilder::delegation([]).audience(Did::new([0xde; 33])).subject(Did::from_secret_key(subject))
    }

    // Same as the above but for invocations
    fn invocation(subject: &SecretKey) -> NucTokenBuilder {
        NucTokenBuilder::invocation(Default::default())
            .audience(Did::new([0xde; 33]))
            .subject(Did::from_secret_key(subject))
    }

    #[rstest]
    #[case::operator(policy::op::eq(".field", json!(42)), PolicyTreeProperties { max_depth: 1, max_policy_width: 1 })]
    #[case::any_od(policy::op::any_of(".field", &[json!(42), json!(1337)]), PolicyTreeProperties { max_depth: 1, max_policy_width: 2 })]
    #[case::not(policy::op::not(policy::op::eq(".field", json!(42))), PolicyTreeProperties { max_depth: 2, max_policy_width: 1 })]
    #[case::and_single(policy::op::and(&[policy::op::eq(".field", json!(42))]), PolicyTreeProperties { max_depth: 2, max_policy_width: 1 })]
    #[case::or_single(policy::op::or(&[policy::op::eq(".field", json!(42))]), PolicyTreeProperties { max_depth: 2, max_policy_width: 1 })]
    #[case::and_multi(
        policy::op::and(&[policy::op::eq(".field", json!(42)), policy::op::eq(".field", json!(42))]),
        PolicyTreeProperties { max_depth: 2, max_policy_width: 2 }
    )]
    #[case::or_multi(
        policy::op::or(&[policy::op::eq(".field", json!(42)), policy::op::eq(".field", json!(42))]),
        PolicyTreeProperties { max_depth: 2, max_policy_width: 2 }
    )]
    #[case::nested(
        policy::op::and(&[
            policy::op::not(policy::op::eq(".field", json!(42))),
            policy::op::any_of(".field", &[json!(42), json!(1337)])
        ]),
        PolicyTreeProperties { max_depth: 3, max_policy_width: 2 }
    )]
    fn policy_properties(#[case] policy: Policy, #[case] expected: PolicyTreeProperties) {
        let properties = PolicyTreeProperties::from(&policy);
        assert_eq!(properties, expected);
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
        Asserter::default().assert_failure(envelope, ValidationKind::UnchainedProofs);
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
        let mut key2_bytes = key1.to_bytes();
        key2_bytes[0] ^= 1;
        let key2 = SecretKey::from_bytes(&key2_bytes).unwrap();

        let root = delegation(&key1).command(["nil"]).issued_by_root();
        let last = delegation(&key2).command(["nil"]).issued_by(key2);
        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::DifferentSubjects);
    }

    #[test]
    fn issuer_audience_mismatch() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().audience(Did::new([0xaa; 33])).issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer { chain_issuer_audience: false }.chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::IssuerAudienceMismatch);
    }

    #[test]
    fn invalid_audience_invocation() {
        let key = secret_key();
        let expected_did = Did::new([0xaa; 33]);
        let actual_did = Did::new([0xbb; 33]);
        let root = delegation(&key).command(["nil"]).issued_by_root();
        let last = invocation(&key).command(["nil"]).audience(actual_did).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Invocation(expected_did),
            ..Default::default()
        };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::InvalidAudience);
    }

    #[test]
    fn invalid_signature() {
        let key = secret_key();
        let root = delegation(&key).command(["nil"]).issued_by_root();
        let token = Chainer::default().chain([root]).encode();
        let (base, signature) = token.rsplit_once(".").unwrap();

        // Change a byte in the signature
        let signature = from_base64(signature).unwrap();
        let mut signature = signature.clone();
        signature[0] ^= 1;
        let signature = to_base64(&signature);
        let token = format!("{base}.{signature}");
        let envelope = NucTokenEnvelope::decode(&token).expect("decode failed");
        let mut asserter = Asserter::default();
        // Don't require a root key signature
        asserter.root_keys = Vec::new();
        asserter.assert_failure(envelope, ValidationKind::InvalidSignatures);
    }

    #[test]
    fn invalid_audience_delegation() {
        let key = secret_key();
        let expected_did = Did::new([0xaa; 33]);
        let actual_did = Did::new([0xbb; 33]);
        let root = delegation(&key).command(["nil"]).issued_by_root();
        let last = delegation(&key).command(["nil"]).audience(actual_did).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Delegation(expected_did),
            ..Default::default()
        };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::InvalidAudience);
    }

    #[test]
    fn missing_proof() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let token = Chainer::default().chain([base.clone().issued_by_root(), base.clone().issued_by(key)]).encode();
        // Keep the token without its proof
        let token = token.split_once("/").unwrap().0;
        let envelope = NucTokenEnvelope::decode(token).expect("invalid token");
        Asserter::default().assert_failure(envelope, ValidationKind::MissingProof);
    }

    #[test]
    fn need_delegation() {
        let key = secret_key();
        let root = delegation(&key).command(["nil"]).issued_by_root();
        let last = invocation(&key).command(["nil"]).issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Delegation(Did::new([0xaa; 33])),
            ..Default::default()
        };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NeedDelegation);
    }

    #[test]
    fn need_invocation() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.issued_by(key);
        let envelope = Chainer::default().chain([root, last]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Invocation(Did::new([0xaa; 33])),
            ..Default::default()
        };
        Asserter::new(parameters).assert_failure(envelope, ValidationKind::NeedInvocation);
    }

    #[test]
    fn not_before_backwards() {
        let now = DateTime::from_timestamp(0, 0).unwrap();
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
        let key = secret_key();
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .issued_by_root();
        let invocation = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject)
            .audience(Did::new([0xaa; 33]))
            .command(["nil"])
            .issued_by(key);

        let envelope = Chainer::default().chain([root, invocation]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyNotMet);
    }

    #[test]
    fn last_policy_not_met() {
        let subject_key = secret_key();
        let subject = Did::from_secret_key(&subject_key);
        let root = NucTokenBuilder::delegation([]).subject(subject.clone()).command(["nil"]).issued_by_root();
        let intermediate = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .issued_by(subject_key);
        let invocation = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject)
            .audience(Did::new([0xaa; 33]))
            .command(["nil"])
            .issued_by(secret_key());

        let envelope = Chainer::default().chain([root, intermediate, invocation]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyNotMet);
    }

    #[test]
    fn policy_too_deep() {
        // Build a not(not(not...(op))) chain that's one too deep
        let mut policy = policy::op::eq(".foo", json!(42));
        for _ in 0..MAX_POLICY_DEPTH {
            policy = policy::op::not(policy);
        }
        let key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::delegation([policy]).subject(subject.clone()).command(["nil"]).issued_by_root();
        let tail = delegation(&key).command(["nil"]).issued_by(key);

        let envelope = Chainer::default().chain([root, tail]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyTooDeep);
    }

    #[test]
    fn policy_array_too_wide() {
        let policy = vec![policy::op::eq(".foo", json!(42)); MAX_POLICY_WIDTH + 1];
        let key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::delegation(policy).subject(subject.clone()).command(["nil"]).issued_by_root();
        let tail = delegation(&key).command(["nil"]).issued_by(key);
        let envelope = Chainer::default().chain([root, tail]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyTooWide);
    }

    #[test]
    fn policy_too_wide() {
        let policy = vec![policy::op::eq(".foo", json!(42)); MAX_POLICY_WIDTH + 1];
        let policy = policy::op::and(policy);
        let key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::delegation([policy]).subject(subject.clone()).command(["nil"]).issued_by_root();
        let tail = delegation(&key).command(["nil"]).issued_by(key);
        let envelope = Chainer::default().chain([root, tail]);
        Asserter::default().assert_failure(envelope, ValidationKind::PolicyTooWide);
    }

    #[test]
    fn proofs_must_be_delegations() {
        let key = secret_key();
        let subject = Did::from_secret_key(&key);
        let root = NucTokenBuilder::invocation(json!({"bar": 1337}).as_object().cloned().unwrap())
            .subject(subject.clone())
            .command(["nil"])
            .issued_by_root();
        let last = NucTokenBuilder::delegation([policy::op::eq(".foo", json!(42))])
            .subject(subject)
            .audience(Did::new([0xaa; 33]))
            .command(["nil"])
            .issued_by(key);

        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::ProofsMustBeDelegations);
    }

    #[test]
    fn root_key_signature_missing() {
        let key = secret_key();
        let base = delegation(&key).command(["nil"]);
        let root = base.clone().issued_by(key.clone());
        let last = base.issued_by(key);

        let envelope = Chainer::default().chain([root, last]);
        Asserter::default().assert_failure(envelope, ValidationKind::RootKeySignatureMissing);
    }

    #[test]
    fn subject_not_in_chain() {
        let subject_key = secret_key();
        let key = secret_key();
        let base = delegation(&subject_key).command(["nil"]);
        let root = base.clone().issued_by_root();
        let last = base.issued_by(key);

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
        let rpc_did = Did::new([0xaa; 33]);
        let root = NucTokenBuilder::delegation([policy::op::eq(".args.foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .issued_by_root();
        let intermediate = NucTokenBuilder::delegation([policy::op::eq(".args.bar", json!(1337))])
            .subject(subject.clone())
            .command(["nil", "bar"])
            .issued_by(subject_key);
        let invocation_key = secret_key();
        let invocation = NucTokenBuilder::invocation(json!({"foo": 42, "bar": 1337}).as_object().cloned().unwrap())
            .subject(subject.clone())
            .audience(rpc_did.clone())
            .command(["nil", "bar", "foo"])
            .issued_by(invocation_key.clone());

        let envelope = Chainer::default().chain([root, intermediate, invocation]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Invocation(rpc_did),
            ..Default::default()
        };
        let ValidatedNucToken { token, proofs } = Asserter::new(parameters).assert_success(envelope);
        assert_eq!(token.issuer, Did::from_secret_key(&invocation_key));

        // Ensure the order is right
        assert_eq!(proofs[0].issuer, subject);
        assert_eq!(proofs[1].issuer, Did::from_secret_key(&ROOT_SECRET_KEYS[0]));
    }

    #[test]
    fn valid_revocation_token() {
        let subject_key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&subject_key);
        let rpc_did = Did::new([0xaa; 33]);
        let root = NucTokenBuilder::delegation([policy::op::eq(".args.foo", json!(42))])
            .subject(subject.clone())
            .command(["nil"])
            .issued_by_root();
        let invocation = NucTokenBuilder::invocation(json!({"foo": 42, "bar": 1337}).as_object().cloned().unwrap())
            .subject(subject.clone())
            .audience(rpc_did.clone())
            .command(["nuc", "revoke"])
            .issued_by(subject_key);

        let envelope = Chainer::default().chain([root, invocation]);
        let parameters = ValidationParameters {
            token_requirements: TokenTypeRequirements::Invocation(rpc_did),
            ..Default::default()
        };
        Asserter::new(parameters).assert_success(envelope);
    }

    #[test]
    fn valid_no_root_keys_needed() {
        let subject_key = SecretKey::random(&mut rand::thread_rng());
        let subject = Did::from_secret_key(&subject_key);
        let token = NucTokenBuilder::delegation([])
            .subject(subject.clone())
            .audience(subject)
            .command(["nil"])
            .issued_by(subject_key);

        let envelope = Chainer::default().chain([token]);
        let mut asserter = Asserter::default();
        asserter.root_keys = Vec::new();
        asserter.assert_success(envelope);
    }
}
