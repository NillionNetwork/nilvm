use super::HandlerResult;
use crate::args::{Cli, InspectNucArgs, MintNucArgs, NucCommand, ValidateNucArgs};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{error::ErrorKind, CommandFactory};
use nillion_nucs::{
    builder::NucTokenBuilder,
    envelope::{DecodedNucToken, NucTokenEnvelope, SignaturesValidated},
    k256::{self, SecretKey},
    policy::Policy,
    token::{NucToken, ProofHash, TokenBody},
    validator::NucValidator,
};
use serde::Serialize;
use std::io::{stdin, Read};
use tools_config::{
    client::ClientParameters,
    identities::{Identity, Kind},
    ToolConfig,
};

pub struct NucHandler {
    parameters: ClientParameters,
}

impl NucHandler {
    pub fn new(parameters: ClientParameters) -> Self {
        Self { parameters }
    }

    pub fn handle(self, command: NucCommand) -> HandlerResult {
        match command {
            NucCommand::Inspect(args) => Self::inspect(args),
            NucCommand::Validate(args) => Self::validate(args),
            NucCommand::Mint(args) => self.mint(args),
        }
    }

    fn read_nuc(nuc: String) -> Result<String> {
        if nuc == "-" {
            let mut buffer = String::new();
            stdin().read_to_string(&mut buffer)?;
            Ok(buffer.trim().to_string())
        } else {
            Ok(nuc)
        }
    }

    fn inspect(args: InspectNucArgs) -> HandlerResult {
        #[derive(Serialize)]
        struct PrettyNuc {
            token: WrappedNucToken,
            proofs: Vec<WrappedNucToken>,
        }

        #[derive(Serialize)]
        struct WrappedNucToken {
            // Same s a NUC token but also contains its hash for easier correlation
            hash: ProofHash,
            #[serde(flatten)]
            token: NucToken,
        }

        impl From<DecodedNucToken> for WrappedNucToken {
            fn from(token: DecodedNucToken) -> Self {
                let hash = token.compute_hash();
                Self { hash, token: token.into_token() }
            }
        }

        let InspectNucArgs { nuc } = args;
        let nuc = Self::read_nuc(nuc).context("reading NUC from stdint")?;
        let envelope = NucTokenEnvelope::decode(&nuc)?;
        let (token, proofs) = envelope.into_parts();
        let token = token.into();
        let proofs = proofs.into_iter().map(WrappedNucToken::from).collect();
        Ok(Box::new(PrettyNuc { token, proofs }))
    }

    fn validate(args: ValidateNucArgs) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            success: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            error: Option<String>,
        }

        let ValidateNucArgs { nuc, root_public_keys } = args;
        let nuc = Self::read_nuc(nuc).context("reading NUC from stdint")?;
        let root_public_keys: Vec<_> = root_public_keys
            .into_iter()
            .map(|pk| k256::PublicKey::from_sec1_bytes(&pk.0))
            .collect::<Result<_, _>>()
            .context("invalid root public key")?;
        let validator = NucValidator::new(&root_public_keys);
        let envelope = match NucTokenEnvelope::decode(&nuc) {
            Ok(envelope) => envelope,
            Err(e) => {
                return Ok(Box::new(Output { success: false, error: Some(format!("invalid envelope: {e}")) }));
            }
        };
        let result = validator.validate(envelope, Default::default());
        let output = match result {
            Ok(_) => Output { success: true, error: None },
            Err(e) => Output { success: false, error: Some(e.to_string()) },
        };
        Ok(Box::new(output))
    }

    fn mint(self, args: MintNucArgs) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            token: String,
        }

        let MintNucArgs {
            extending,
            audience,
            subject,
            expires_at,
            expires_in,
            not_before,
            command,
            metadata,
            nonce,
            proof,
            invocation,
            delegation,
        } = args;
        let mut builder = match extending {
            Some(token) => {
                let base = Self::parse_nuc_envelope(&token, "--extending");
                let mut builder = NucTokenBuilder::extending(base)?;
                if invocation.is_some() || delegation.is_some() {
                    let body = Self::build_token_body(invocation, delegation)?;
                    builder = builder.body(body);
                }
                builder
            }
            None => {
                let body = Self::build_token_body(invocation, delegation)?;
                NucTokenBuilder::new(body)
            }
        };
        builder = builder.audience(audience);
        if let Some(subject) = subject {
            builder = builder.subject(subject);
        }
        if let Some(timestamp) = expires_at {
            let timestamp = Self::parse_timestamp(timestamp, "expires_at");
            builder = builder.expires_at(timestamp);
        }
        if let Some(offset) = expires_in {
            builder = builder.expires_in(offset.into());
        }
        if let Some(timestamp) = not_before {
            let timestamp = Self::parse_timestamp(timestamp, "not_before");
            builder = builder.not_before(timestamp);
        }
        if let Some(command) = command {
            builder = builder.command(command);
        }
        if let Some(metadata) = metadata {
            let metadata = serde_json::from_str(&metadata).context("invalid metadata")?;
            builder = builder.meta(metadata);
        }
        if let Some(nonce) = nonce {
            builder = builder.nonce(nonce.0);
        }
        if let Some(proof) = proof {
            let proof = Self::parse_nuc_envelope(&proof, "--proof");
            builder = builder.proof(proof);
        }
        let identity = Identity::read_from_config(&self.parameters.identity)?;
        let key = match identity.kind {
            Kind::Secp256k1 => SecretKey::from_slice(&identity.private_key)?,
            Kind::Ed25519 => bail!("ed25519 not supported"),
        };
        let token = builder.build(&key.into()).context("failed to build token")?;
        Ok(Box::new(Output { token }))
    }

    fn parse_nuc_envelope(token: &str, parameter: &str) -> NucTokenEnvelope<SignaturesValidated> {
        let token = match NucTokenEnvelope::decode(token) {
            Ok(token) => token,
            Err(e) => {
                Cli::command().error(ErrorKind::InvalidValue, format!("invalid token in `{parameter}`: {e}")).exit();
            }
        };
        match token.validate_signatures() {
            Ok(token) => token,
            Err(e) => {
                Cli::command()
                    .error(ErrorKind::InvalidValue, format!("invalid signatures in token in `{parameter}`: {e}"))
                    .exit();
            }
        }
    }

    fn parse_timestamp(timestamp: i64, parameter: &str) -> DateTime<Utc> {
        match DateTime::from_timestamp(timestamp, 0) {
            Some(timestamp) => timestamp,
            None => {
                Cli::command().error(ErrorKind::InvalidValue, format!("invalid timestamp in `{parameter}`")).exit();
            }
        }
    }

    fn build_token_body(invocation: Option<String>, delegation: Option<String>) -> Result<TokenBody> {
        match (invocation, delegation) {
            (Some(args), None) => {
                let args = serde_json::from_str(&args).context("invalid arguments")?;
                Ok(TokenBody::Invocation(args))
            }
            (None, Some(policies)) => {
                let policies: Vec<Policy> = serde_json::from_str(&policies).context("invalid policies")?;
                Ok(TokenBody::Delegation(policies))
            }
            (None, None) => {
                Cli::command().error(ErrorKind::InvalidValue, "need one of --invocation or --delegation").exit();
            }
            _ => unreachable!("only one of invocation/delegation enforced via clap "),
        }
    }
}
