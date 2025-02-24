use crate::policy::Policy;
use chrono::{DateTime, Utc};
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt, str::FromStr};

/// A JSON object.
pub type JsonObject = serde_json::Map<String, serde_json::Value>;

/// A Nillion NUC token.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NucToken {
    /// The token issuer.
    #[serde(rename = "iss")]
    pub issuer: Did,

    /// The token issuer.
    #[serde(rename = "aud")]
    pub audience: Did,

    /// The token subject.
    #[serde(rename = "sub")]
    pub subject: Did,

    /// The first timestamp at which this token is valid.
    #[serde(rename = "nbf", default)]
    pub not_before: Option<DateTime<Utc>>,

    /// The timestamp at which this token becomes invalid.
    #[serde(rename = "exp", default)]
    pub expires_at: Option<DateTime<Utc>>,

    /// The command that is being invoked or the authority is being delegated for.
    #[serde(rename = "cmd")]
    pub command: Command,

    /// The token body.
    #[serde(flatten)]
    pub body: TokenBody,

    /// Metadata associated to this token.
    #[serde(default)]
    pub meta: Option<JsonObject>,

    /// The hash of the proofs in this token.
    #[serde(rename = "prf", default)]
    pub proofs: Vec<ProofHash>,
}

/// A decentralized ID.
#[derive(Clone, Debug, PartialEq, SerializeDisplay, DeserializeFromStr)]
pub struct Did {
    /// The method.
    pub method: String,

    /// The public key.
    pub public_key: [u8; 33],
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { method, public_key } = self;
        let public_key = hex::encode(public_key);
        write!(f, "did:{method}:{public_key}")
    }
}

impl FromStr for Did {
    type Err = ParseDidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.strip_prefix("did:").ok_or(ParseDidError::NoDid)?;
        let (method, raw_public_key) = s.split_once(':').ok_or(ParseDidError::NoMethod)?;
        let mut public_key = [0; 33];
        hex::decode_to_slice(raw_public_key, &mut public_key).map_err(ParseDidError::PublicKeyChars)?;
        Ok(Self { method: method.to_string(), public_key })
    }
}

/// An error when parsing a DID.
#[derive(Debug, thiserror::Error)]
pub enum ParseDidError {
    #[error("no 'did' prefix")]
    NoDid,

    #[error("no method in did")]
    NoMethod,

    #[error("invalid public key: {0}")]
    PublicKeyChars(FromHexError),
}

/// The hash of a proof.
#[derive(Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq)]
pub struct ProofHash([u8; 32]);

impl fmt::Display for ProofHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hash = hex::encode(self.0);
        write!(f, "{hash}")
    }
}

impl FromStr for ProofHash {
    type Err = FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut data = [0; 32];
        hex::decode_to_slice(s, &mut data)?;
        Ok(Self(data))
    }
}

/// A command.
#[derive(Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq)]
pub struct Command(Vec<String>);

impl FromStr for Command {
    type Err = MalformedCommandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('/') {
            return Err(MalformedCommandError::LeadingSlash);
        }
        let s = &s[1..];
        if s.is_empty() {
            return Ok(Self(vec![]));
        }
        let mut segments = Vec::new();
        for segment in s.split("/") {
            if segment.is_empty() {
                return Err(MalformedCommandError::EmptySegment);
            }
            segments.push(segment.into());
        }
        Ok(Self(segments))
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, "/");
        }
        for segment in &self.0 {
            write!(f, "/{segment}")?;
        }
        Ok(())
    }
}

/// The body of a token
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TokenBody {
    #[serde(rename = "pol")]
    Delegation(Vec<Policy>),

    #[serde(rename = "args")]
    Invocation(JsonObject),
}

/// An encountered error when parsing a command.
#[derive(Debug, thiserror::Error)]
pub enum MalformedCommandError {
    #[error("no leading slash")]
    LeadingSlash,

    #[error("empty segment")]
    EmptySegment,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy;
    use rstest::rstest;
    use serde_json::json;

    #[rstest]
    #[case::root("/", &[])]
    #[case::one("/nil", &["nil"])]
    #[case::two("/nil/bar", &["nil", "bar"])]
    fn parse_valid_commands(#[case] input: &str, #[case] expected: &[&str]) {
        let parsed: Command = input.parse().expect("parsing failed");
        assert_eq!(&parsed.0, expected);
        assert_eq!(parsed.to_string(), input);
    }

    #[rstest]
    #[case::empty("")]
    #[case::leading_double_slash("//")]
    #[case::trailing_slash("/nil/")]
    #[case::double_slash_in_middle("/nil//a")]
    fn parse_invalid_commands(#[case] input: &str) {
        input.parse::<Command>().expect_err("parsing succeeded");
    }

    #[test]
    fn parse_valid_proof_hash() {
        let input = "f4f04af6a832bcd8a6855df5d0242c9a71e9da17faeb2d33b30c8903f1b5a944";
        let hash: ProofHash = input.parse().expect("parse failed");
        assert_eq!(
            &hash.0,
            b"\xf4\xf0J\xf6\xa82\xbc\xd8\xa6\x85]\xf5\xd0$,\x9aq\xe9\xda\x17\xfa\xeb-3\xb3\x0c\x89\x03\xf1\xb5\xa9D"
        );
        assert_eq!(hash.to_string(), input);
    }

    #[test]
    fn parse_valid_did() {
        let input = "did:test:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let did: Did = input.parse().expect("parse failed");
        assert_eq!(did.method, "test");
        assert_eq!(&did.public_key, b"\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa");
        assert_eq!(did.to_string(), input);
    }

    #[rstest]
    #[case::no_did("foo:bar:aa")]
    #[case::no_method("did:bar")]
    #[case::trailing_colon("did:bar:aa:")]
    #[case::invalid_public_key("did:bar:lol")]
    fn parse_invalid_did(#[case] input: &str) {
        Did::from_str(input).expect_err("parse succeeded");
    }

    #[test]
    fn parse_minimal_delegation() {
        let input = r#"
{
  "iss": "did:nil:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "aud": "did:nil:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "sub": "did:nil:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "cmd": "/nil/db/read",
  "pol": [
    ["==", ".foo", 42]
  ]
}"#;
        serde_json::from_str::<NucToken>(input).expect("parsing failed");
    }

    #[test]
    fn parse_full_delegation() {
        let input = r#"
{
  "iss": "did:nil:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "aud": "did:nil:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "sub": "did:nil:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "cmd": "/nil/db/read",
  "nbf": "2025-02-24T10:39:12.282054Z",
  "exp": "2025-02-24T12:39:12.282054Z",
  "pol": [
    ["==", ".foo", 42]
  ],
  "meta": {
    "name": "bob"
  },
  "prf": ["f4f04af6a832bcd8a6855df5d0242c9a71e9da17faeb2d33b30c8903f1b5a944"]
}"#;
        let token: NucToken = serde_json::from_str(input).expect("parsing failed");
        let expected = NucToken {
            issuer: Did { method: "nil".into(), public_key: *b"\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa" },
            audience: Did { method: "nil".into(), public_key: *b"\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb" },
            subject: Did { method: "nil".into(), public_key: *b"\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc" },
            not_before: Some("2025-02-24T10:39:12.282054Z".parse().expect("invalid date")),
            expires_at: Some("2025-02-24T12:39:12.282054Z".parse().expect("invalid date")),
            command: Command(vec!["nil".into(), "db".into(), "read".into()]),
            body: TokenBody::Delegation(vec![policy::op::eq(".foo", json!(42))]),
            proofs: vec![ProofHash(*b"\xf4\xf0J\xf6\xa82\xbc\xd8\xa6\x85]\xf5\xd0$,\x9aq\xe9\xda\x17\xfa\xeb-3\xb3\x0c\x89\x03\xf1\xb5\xa9D")],
            meta: Some(json!({ "name": "bob" }).as_object().cloned().unwrap()),
        };
        assert_eq!(token, expected);

        // Ensure `token -> string -> token` gives us back the original token
        let serialized = serde_json::to_string(&token).expect("serialize failed");
        let deserialized: NucToken = serde_json::from_str(&serialized).expect("deserialize failed");
        assert_eq!(deserialized, token);
    }

    #[test]
    fn parse_minimal_invocation() {
        let input = r#"
{
  "iss": "did:nil:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "aud": "did:nil:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "sub": "did:nil:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "cmd": "/nil/db/read",
  "args": {
    "bar": 42
  }
}"#;
        serde_json::from_str::<NucToken>(input).expect("parsing failed");
    }

    #[test]
    fn parse_full_invocation() {
        let input = r#"
{
  "iss": "did:nil:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "aud": "did:nil:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "sub": "did:nil:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "cmd": "/nil/db/read",
  "nbf": "2025-02-24T10:39:12.282054Z",
  "exp": "2025-02-24T12:39:12.282054Z",
  "args": {
    "foo": 42
  },
  "meta": {
    "name": "bob"
  },
  "prf": ["f4f04af6a832bcd8a6855df5d0242c9a71e9da17faeb2d33b30c8903f1b5a944"]
}"#;
        let token: NucToken = serde_json::from_str(input).expect("parsing failed");
        let expected = NucToken {
            issuer: Did { method: "nil".into(), public_key: *b"\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa" },
            audience: Did { method: "nil".into(), public_key: *b"\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb\xbb" },
            subject: Did { method: "nil".into(), public_key: *b"\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc\xcc" },
            not_before: Some("2025-02-24T10:39:12.282054Z".parse().expect("invalid date")),
            expires_at: Some("2025-02-24T12:39:12.282054Z".parse().expect("invalid date")),
            command: Command(vec!["nil".into(), "db".into(), "read".into()]),
            body: TokenBody::Invocation(json!({ "foo": 42 }).as_object().cloned().unwrap()),
            proofs: vec![ProofHash(*b"\xf4\xf0J\xf6\xa82\xbc\xd8\xa6\x85]\xf5\xd0$,\x9aq\xe9\xda\x17\xfa\xeb-3\xb3\x0c\x89\x03\xf1\xb5\xa9D")],
            meta: Some(json!({ "name": "bob" }).as_object().cloned().unwrap()),
        };
        assert_eq!(token, expected);

        // Ensure `token -> string -> token` gives us back the original token
        let serialized = serde_json::to_string(&token).expect("serialize failed");
        let deserialized: NucToken = serde_json::from_str(&serialized).expect("deserialize failed");
        assert_eq!(deserialized, token);
    }
}
