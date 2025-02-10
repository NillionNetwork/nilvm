//! Implementation of the program model and the repository that contains them.

use crate::storage::repositories::blob::{BinarySerde, BinarySerdeError};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use node_api::auth::rust::UserId;
use prost::Message;
use std::{fmt, str::FromStr};

mod proto {
    tonic::include_proto!("node.programs.v1.programs");
}

/// A Nillion Execution VM MIR program.
#[derive(Clone)]
pub struct ProgramModel {
    /// The program's mir.
    pub mir: ProgramMIR,
}

impl BinarySerde for ProgramModel {
    fn serialize(self) -> Result<Vec<u8>, BinarySerdeError> {
        let proto_mir = self.mir.into_proto().encode_to_vec();
        let model = proto::Program { proto_mir };
        Ok(model.encode_to_vec())
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, BinarySerdeError> {
        let proto::Program { proto_mir } =
            proto::Program::decode(bytes).map_err(|e| BinarySerdeError(format!("invalid protobuf model: {e}")))?;
        let mir = ProgramMIR::try_decode(&proto_mir)
            .map_err(|e| BinarySerdeError(format!("failed to convert protobuf to MIR: {e}")))?;
        Ok(Self { mir })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProgramId {
    Builtin(String),
    Uploaded { user_id: UserId, name: String, sha256: Vec<u8> },
}

impl FromStr for ProgramId {
    type Err = ParseProgramIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseProgramIdError::*;
        let (prefix, rest) = s.split_once("/").ok_or(NoPrefix)?;
        if prefix == "builtin" {
            return Ok(Self::Builtin(rest.to_string()));
        }
        let user_id = prefix.parse().map_err(|_| InvalidUserId)?;
        let (name, rest) = rest.split_once("/").ok_or(NoHashType)?;
        let (hash_type, hash) = rest.split_once("/").ok_or(NoHash)?;
        let hash = hex::decode(hash).map_err(|_| InvalidHash)?;
        match hash_type {
            "sha256" => Ok(Self::Uploaded { user_id, name: name.to_string(), sha256: hash }),
            _ => Err(InvalidHashType),
        }
    }
}

impl fmt::Display for ProgramId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(name) => write!(f, "builtin/{name}"),
            Self::Uploaded { user_id, name, sha256 } => write!(f, "{user_id}/{name}/sha256/{}", hex::encode(sha256)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ParseProgramIdError {
    #[error("no '/' prefix in program")]
    NoPrefix,

    #[error("invalid user id")]
    InvalidUserId,

    #[error("no hash type")]
    NoHashType,

    #[error("no hash")]
    NoHash,

    #[error("invalid hash")]
    InvalidHash,

    #[error("invalid hash type")]
    InvalidHashType,
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::builtin("builtin/tecdsa_sign", ProgramId::Builtin("tecdsa_sign".into()))]
    #[case::uploaded(
        "1234567890abcdef1234567890abcdef12345678/test/sha256/0102030401020304010203040102030401020304010203040102030401020304",
        ProgramId::Uploaded{
            user_id: UserId::from(*b"\x124Vx\x90\xab\xcd\xef\x124Vx\x90\xab\xcd\xef\x124Vx"),
            name: "test".into(),
            sha256: b"\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04\x01\x02\x03\x04".into()
        }
    )]
    fn parse_program_id(#[case] input: &str, #[case] id: ProgramId) {
        let parsed: ProgramId = input.parse().expect("invalid input");
        assert_eq!(parsed, id);

        // Ensure parse -> to_string gives back the original
        assert_eq!(parsed.to_string(), input);
    }
}
