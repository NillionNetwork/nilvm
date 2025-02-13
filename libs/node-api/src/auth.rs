//! Authentication messages.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::auth::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use std::{fmt, str::FromStr};

    use crate::{errors::InvalidHexId, membership::rust::NodeId, ConvertProto, ProtoError, TryIntoRust};
    use chrono::{DateTime, Utc};
    use sha2::{Digest, Sha256};

    /// A public key.
    #[derive(Clone, Debug, PartialEq)]
    pub enum PublicKey {
        /// An ED25519 key.
        Ed25519([u8; 32]),

        /// A SECP256K1 key.
        Secp256k1([u8; 33]),
    }

    impl ConvertProto for PublicKey {
        type ProtoType = super::proto::public_key::PublicKey;

        fn into_proto(self) -> Self::ProtoType {
            match self {
                Self::Ed25519(contents) => Self::ProtoType {
                    key_type: super::proto::public_key::PublicKeyType::Ed25519.into(),
                    contents: contents.to_vec(),
                },
                Self::Secp256k1(contents) => Self::ProtoType {
                    key_type: super::proto::public_key::PublicKeyType::Secp256k1.into(),
                    contents: contents.to_vec(),
                },
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let key_type = super::proto::public_key::PublicKeyType::try_from(model.key_type)
                .map_err(|_| ProtoError("invalid public key type"))?;
            match key_type {
                super::proto::public_key::PublicKeyType::Ed25519 => {
                    let contents = model.contents.try_into().map_err(|_| ProtoError("invalid public key"))?;
                    Ok(Self::Ed25519(contents))
                }
                super::proto::public_key::PublicKeyType::Secp256k1 => {
                    let contents = model.contents.try_into().map_err(|_| ProtoError("invalid public key"))?;
                    Ok(Self::Secp256k1(contents))
                }
            }
        }
    }

    /// A token.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Token {
        /// The nonce
        pub nonce: [u8; 32],

        /// The target node's identity.
        pub target_identity: NodeId,

        /// The time at which this token expires.
        pub expires_at: DateTime<Utc>,
    }

    impl ConvertProto for Token {
        type ProtoType = super::proto::token::Token;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                nonce: self.nonce.to_vec(),
                target_identity: Some(self.target_identity.into_proto()),
                expires_at: Some(self.expires_at.into_proto()),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let nonce = model.nonce.try_into().map_err(|_| ProtoError("'nonce' must be 32 bytes long"))?;
            let target_identity =
                model.target_identity.ok_or(ProtoError("'target_identity' not set"))?.try_into_rust()?;
            let expires_at = model
                .expires_at
                .ok_or(ProtoError("'expires_at' not set"))?
                .try_into_rust()
                .map_err(|_| ProtoError("invalid 'expires_at' field"))?;
            Ok(Self { nonce, target_identity, expires_at })
        }
    }

    /// A signed token.
    #[derive(Clone, Debug, PartialEq)]
    pub struct SignedToken {
        /// A [Token] serialized into bytes.
        pub serialized_token: Vec<u8>,

        /// The public key for the private key this token is signed with.
        pub public_key: PublicKey,

        /// The serialized token signature.
        pub signature: Vec<u8>,
    }

    impl ConvertProto for SignedToken {
        type ProtoType = super::proto::token::SignedToken;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                serialized_token: self.serialized_token,
                public_key: Some(self.public_key.into_proto()),
                signature: self.signature,
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let Self::ProtoType { serialized_token, public_key, signature } = model;
            let public_key = public_key.ok_or(ProtoError("'public_key' not set"))?;
            Ok(Self { serialized_token, public_key: public_key.try_into_rust()?, signature })
        }
    }

    /// A user identifier
    #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    pub struct UserId([u8; 20]);

    impl AsRef<[u8]> for UserId {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    impl UserId {
        /// Constructs a new instance from a byte sequence.
        ///
        /// This uses the last 20 bytes of the sha256 hash of the given bytes.
        pub fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Self {
            let hash = Sha256::digest(bytes.as_ref());
            // SAFETY: hash.len() == 32 so this is safe
            let id_input = hash[hash.len() - 20..].try_into().expect("not enough bytes");
            Self(id_input)
        }
    }

    impl From<[u8; 20]> for UserId {
        fn from(id: [u8; 20]) -> Self {
            Self(id)
        }
    }

    impl ConvertProto for UserId {
        type ProtoType = super::proto::user::UserId;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { contents: self.0.to_vec() }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let contents = model.contents.try_into().map_err(|_| ProtoError("invalid user id length"))?;
            Ok(Self(contents))
        }
    }

    impl fmt::Display for UserId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", hex::encode(self.0))
        }
    }

    impl FromStr for UserId {
        type Err = InvalidHexId;

        fn from_str(id: &str) -> Result<Self, Self::Err> {
            let id = hex::decode(id).map_err(|_| InvalidHexId::HexEncoding)?;
            let id = id.try_into().map_err(|_| InvalidHexId::InvalidLength)?;
            Ok(Self(id))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn user_id_display() {
            let user = UserId::from_bytes("bob");
            assert_eq!(user.to_string(), "3113a1170de795e4b725b84d1e0b4cfd9ec58ce9");
        }

        #[test]
        fn parse() {
            let user = UserId::from_str("3113a1170de795e4b725b84d1e0b4cfd9ec58ce9").expect("invalid user");
            assert_eq!(user, UserId::from_bytes("bob"));
        }
    }
}
