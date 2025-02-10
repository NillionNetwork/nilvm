//! Secret sharer implementation.

use crate::{
    party::PartyMapper,
    protocol::{PolyDegree, ShareGenerationError},
};
use basic_types::PartyId;
use math_lib::{
    fields::{BinaryExtField, Field, PrimeField},
    modular::{Prime, SafePrime},
};
use rustc_hash::FxHashMap;

/// Each party's shares.
pub type PartyShares<T> = FxHashMap<PartyId, T>;

/// A type that can perform secret sharing, turning secrets into shares and shares into secrets.
///
/// The generic type S refers to the type used to represent shares.
pub trait SecretSharer<S>: SecretSharerProperties {
    /// The type of the secrets and shares this sharer operates on.
    type Secret;

    /// The error returned during the secret recovery.
    type RecoverError: std::error::Error + Send + Sync + 'static;

    /// The error returned during the hyper map.
    type HyperMapError: std::error::Error;

    /// Generates shares for the given secret.
    fn generate_shares(&self, secret: &Self::Secret, degree: PolyDegree)
    -> Result<PartyShares<S>, GenerateSharesError>;

    /// Recovers the secret behind the provided shares.
    fn recover<I>(&self, shares: I) -> Result<Self::Secret, Self::RecoverError>
    where
        I: IntoIterator<Item = (PartyId, S)>;

    /// Weighs the share with Lagrange coefficient.
    fn weigh(&self, share: S) -> Result<Self::Secret, Self::RecoverError>;

    /// Maps between vectors using hyper-invertible matrix.
    fn hyper_map<I>(&self, shares: I) -> Result<Vec<Self::Secret>, Self::HyperMapError>
    where
        I: IntoIterator<Item = (PartyId, S)>;
}

/// The properties of a secret sharer.
pub trait SecretSharerProperties {
    /// The prime number being used.
    type Prime: Prime;

    /// Gets the local party id.
    fn local_party_id(&self) -> &PartyId;

    /// Gets the list of all party ids this instance generates shares for.
    fn parties(&self) -> Vec<PartyId>;

    /// Gets the number of parties this instance generates shares for.
    fn party_count(&self) -> usize;

    /// Gets the prime shamir's party mapper.
    ///
    /// Note that eventually we should be using the same mapper for both fields so there won't be a prime-specific one.
    fn party_mapper(&self) -> &PartyMapper<PrimeField<Self::Prime>>;

    /// Gets the degree of the polynomial that hides secrets generated through this instance.
    fn polynomial_degree(&self) -> u64;
}

/// Trait that indicates a secret sharer can operate on a particular field.
pub trait FieldSecretSharer<F: Field>:
    SecretSharer<F::Element, Secret = F::Element> + SecretSharer<Vec<F::Element>, Secret = Vec<F::Element>>
{
}

impl<T, F> FieldSecretSharer<F> for T
where
    F: Field,
    T: SecretSharer<F::Element, Secret = F::Element> + SecretSharer<Vec<F::Element>, Secret = Vec<F::Element>>,
{
}

/// A secret sharer that supports secret sharing over a safe prime.
///
/// This should be the preferred trait over `SecretSharer` and `FieldSecretSharer` as this one is a
/// "universal secret sharer" that can be used on any of the fields we support.
pub trait SafePrimeSecretSharer<T: SafePrime>:
    FieldSecretSharer<PrimeField<T>> + FieldSecretSharer<PrimeField<T::SophiePrime>> + FieldSecretSharer<BinaryExtField>
{
}

impl<T: SafePrime, S> SafePrimeSecretSharer<T> for S where
    S: FieldSecretSharer<PrimeField<T>>
        + FieldSecretSharer<PrimeField<T::SophiePrime>>
        + FieldSecretSharer<BinaryExtField>
{
}

/// An error during the recovery of multiple secrets at once.
#[derive(thiserror::Error, Debug)]
pub enum MultiRecoverError<E> {
    /// The number of shares some node provided doesn't match what the rest provided.
    #[error("share count mismatch")]
    ShareCountMismatch,

    /// No shares were provided.
    #[error("no shares provided")]
    NoShares,

    /// The recovery failed.
    #[error(transparent)]
    Recovery(#[from] E),
}

/// An error during the recovery of multiple secrets at once.
#[derive(thiserror::Error, Debug)]
pub enum MultiMapError<E> {
    /// The number of shares some node provided doesn't match what the rest provided.
    #[error("share count mismatch")]
    ShareCountMismatch,

    /// No shares were provided.
    #[error("no shares provided")]
    NoShares,

    /// The recovery failed.
    #[error(transparent)]
    HyperMap(#[from] E),
}

/// An error during the share generation.
#[derive(thiserror::Error, Debug)]
pub enum GenerateSharesError {
    /// An error during the share generation itself.
    #[error("share generation failed: {0}")]
    ShareGeneration(#[from] ShareGenerationError),

    /// An abscissa couldn't be mapped into a party id.
    #[error("abscissa mapping failed")]
    AbscissaMapping,

    /// An invalid operation was attempted.
    #[error("invalid operation: {msg}")]
    InvalidOperation {
        /// The error message.
        msg: String,
    },
}
