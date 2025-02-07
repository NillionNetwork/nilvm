//! nada-value errors
use crate::encoders::EncodeVariableError;
use basic_types::{errors::UnimplementedError, jar::DuplicatePartyShare};
use ecdsa_keypair::{privatekey::EcdsaPrivateKeyError, signature::EcdsaSignatureError};
use math_lib::modular::{DecodeError, Overflow};
use nada_type::{NadaType, TypeError};
use num_bigint::TryFromBigIntError;
use shamir_sharing::secret_sharer::GenerateSharesError;
use std::{num::TryFromIntError, string::FromUtf8Error};
use thiserror::Error;

/// Errors that occur during the encoding
#[derive(Error, Debug)]
pub enum ClearToEncryptedError {
    /// Duplicate party in party jar.
    #[error(transparent)]
    DuplicatePartyShare(#[from] DuplicatePartyShare),

    /// Not enough blinding factor shares.
    #[error("not enough blinding factor shares")]
    NotEnoughBlindingFactorShares,

    /// Type error.
    #[error(transparent)]
    TypeError(#[from] TypeError),

    /// Encoding error.
    #[error(transparent)]
    EncodingError(#[from] EncodingError),

    /// Shares generation error.
    #[error(transparent)]
    GenerateSharesError(#[from] GenerateSharesError),

    /// Overflow
    #[error(transparent)]
    Overflow(#[from] Overflow),

    /// Not enough values.
    #[error("not enough values")]
    NotEnoughValues,

    /// Ecdsa shares generation error.
    #[error(transparent)]
    GenerateEcdsaSharesError(#[from] EcdsaPrivateKeyError),

    /// Ecdsa shares generation error due to large number of parties.
    #[error("Number of parties requested is too large for u16")]
    TooManyPartiesForEcdsaSignature,

    /// Ecdsa shares generation error.
    #[error(transparent)]
    GenerateEcdsaSignatureSharesError(#[from] EcdsaSignatureError),
}

/// Errors that occur during the encoding
#[derive(Error, Debug)]
pub enum EncodingError {
    /// This error occurs when the integer secret supplied by the user is too big and overflow occurs
    #[error("supplied integer secret is too big")]
    PrimeTooSmall,

    /// Value is out of bounds (blob size or rational digit count exceeds u64)
    #[error("out of bounds")]
    OutOfBounds,

    /// Secret for a tuple's branch is not found
    #[error("secret for a tuple's branch not found")]
    SecretBranchNotFound,

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(#[from] UnimplementedError),

    /// Not implemented.
    #[error(transparent)]
    TransformVariableError(#[from] EncodeVariableError),

    /// Type error.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

impl From<Overflow> for EncodingError {
    fn from(_: Overflow) -> Self {
        EncodingError::PrimeTooSmall
    }
}

impl From<PrimeTooSmallError> for EncodingError {
    fn from(_: PrimeTooSmallError) -> Self {
        Self::PrimeTooSmall
    }
}

/// Errors that occur during the decoding
#[derive(Error, Debug)]
pub enum DecodingError {
    /// This error occurs when a data type is tried to decode to unsupported output.
    #[error("unsupported decoding")]
    Unsupported,

    /// This error occurs while a BigUint is decoding
    #[error("bigint error decoding a biguint")]
    FromBigIntError,

    /// This error occurs while a string is decoding
    #[error("utf8 error decoding a string")]
    FromUtf8Error,

    /// An error decoding the underlying modular number.
    #[error("modular number decoding failed")]
    ModularDecoding,

    /// Value is out of bounds (blob size or rational digit count exceeds u64)
    #[error("out of bounds")]
    OutOfBounds,

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(#[from] UnimplementedError),

    /// Not implemented.
    #[error(transparent)]
    TransformVariableError(#[from] EncodeVariableError),

    /// Type error.
    #[error("type error: {0}")]
    Type(#[from] TypeError),
}

impl From<DecodeError> for DecodingError {
    fn from(_: DecodeError) -> Self {
        Self::ModularDecoding
    }
}

impl From<FromUtf8Error> for DecodingError {
    fn from(_: FromUtf8Error) -> Self {
        DecodingError::FromUtf8Error
    }
}

impl<T> From<TryFromBigIntError<T>> for DecodingError {
    fn from(_: TryFromBigIntError<T>) -> Self {
        DecodingError::FromBigIntError
    }
}

/// Error returned during the blob chunk size calculation.
#[derive(Error, Debug)]
#[cfg_attr(test, derive(PartialEq))]
#[error("prime is too small")]
pub struct PrimeTooSmallError;

/// Error returned during the clear to modular conversion.
#[derive(Error, Debug)]
pub enum ClearModularError {
    /// Not enough values.
    #[error("not enough values")]
    NotEnoughValues,

    /// Overflow error.
    #[error(transparent)]
    Overflow(#[from] Overflow),

    /// TypeError error.
    #[error(transparent)]
    TypeError(#[from] TypeError),

    /// Unsupported type error
    #[error("unsupported type {0}")]
    Unsupported(String),
}

/// ModularValue is not a primitive value.
#[derive(Error, Debug)]
#[error("non primitive value")]
pub struct NonPrimitiveValue;

/// Errors that occur during the encoding
#[derive(Error, Debug)]
pub enum EncryptedToClearError {
    /// Encoding error.
    #[error(transparent)]
    EncodingError(#[from] EncodingError),

    /// Party Jar is empty.
    #[error("party jar provided is empty")]
    PartyJarEmpty,

    /// Values provided do not match.
    #[error("provided encrypted {0} do not match")]
    Missmatch(String),

    /// Decode error.
    #[error(transparent)]
    DecodeError(#[from] DecodeError),

    /// Decoding error. We have different decoders ;)
    #[error(transparent)]
    DecodingError(#[from] DecodingError),

    // #[error(transparent)]
    // Recovery(#[from] dyn SecretSharer<S>::RecoverError),
    /// Blob decryption gone wrong.
    #[error("blob decryption gone wrong")]
    BlobChunkSizeError,

    /// Duplicate party in party jar.
    #[error(transparent)]
    DuplicatePartyShare(#[from] DuplicatePartyShare),

    /// Tuple decryption gone wrong.
    #[error("tuple decryption gone wrong")]
    TupleGoneWrong,

    /// Values could not be recovered from shamir shares.
    #[error("{0} could not be recovered from shares")]
    SharedSecretRecovery(String),

    /// TryFromIntError
    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),

    /// Invalid type
    #[error("invalid type: {0}")]
    InvalidType(NadaType),

    /// Unimplemented
    #[error(transparent)]
    Unimplemented(#[from] UnimplementedError),

    /// Wrong blob size
    #[error("wrong blob size")]
    WrongBlobSize,

    /// Party values not found
    #[error("party values not found")]
    PartyValuesNotFound,

    /// Type error
    #[error("not enough values")]
    NotEnoughValues,

    /// Type error
    #[error(transparent)]
    TypeError(#[from] TypeError),

    /// Type error
    #[error("not possible to transform into ecdsa key share type")]
    TransformingIntoEcdsaKeyShareError,
}
