//! The ecdsa signature implementation.

use generic_ec::{curves::Secp256k1, NonZero, Point, Scalar};
use givre::{ciphersuite, ciphersuite::Ciphersuite, signing::aggregate::Signature};
use rand::rngs::OsRng;
use std::{
    cmp::PartialEq,
    fmt,
    ops::{Add, Neg, Sub},
};

use thiserror::Error;

/// A struct representing an EdDSA private key.
/// The private key is a non-zero scalar defined on the ed25519 elliptic curve.
/// Note: EddsaSignature is considered a public value, so we do not implement
/// the corresponding share version, i.e., EddsaSignatureShare.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EddsaSignature {
    /// The signature
    pub signature: Signature<ciphersuite::Ed25519>,
}

impl EddsaSignature {
    /// Returns the serialized length of the Eddsa Signature
    pub fn serialized_len(&self) -> usize {
        ciphersuite::Ed25519::NORMALIZED_POINT_SIZE + ciphersuite::Ed25519::SCALAR_SIZE
    }

    /// Creates an EddsaSignature from its components
    /// Note: z_bytes must be in little-endian byte order
    pub fn from_components_bytes(r_bytes: &[u8], z_bytes: &[u8]) -> Result<Self, EddsaSignatureError> {
        let z = Scalar::from_le_bytes(z_bytes).map_err(|e| {
            EddsaSignatureError::InvalidComponentSignature(format!("invalid signature z component: {e}"))
        })?;

        let r_point = Point::from_bytes(r_bytes).map_err(|e| {
            EddsaSignatureError::InvalidComponentSignature(format!(
                "invalid byte reconstruction of signature point r: {e}"
            ))
        })?;
        let r = ciphersuite::NormalizedPoint::try_normalize(r_point).map_err(|_| {
            EddsaSignatureError::InvalidComponentSignature(
                "invalid reconstruction of signature normalized point z".to_string(),
            )
        })?;

        Ok(Self { signature: Signature { r, z } })
    }
}

impl fmt::Display for EddsaSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EddsaSignature {{ R: {:?}, z: {:?} }}", self.signature.r, self.signature.z)
    }
}

/// A struct representing an ECDSA private key.
/// The private key is a non-zero scalar defined on the secp256k1 elliptic curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EcdsaSignature {
    /// r component of the signature
    pub r: NonZero<Scalar<Secp256k1>>,
    /// s component of the signature
    pub s: NonZero<Scalar<Secp256k1>>,
}

impl EcdsaSignature {
    /// Normalizes the signature
    ///
    /// Given that $(r, s)$ is valid signature, $(r, -s)$ is also a valid signature. Some applications (like Bitcoin)
    /// remove this ambiguity by restricting $s$ to be in lower half. This method normailizes the signature by picking
    /// $s$ that is in lower half.
    ///
    /// Note that signing protocol implemented within this crate outputs normalized signature by default.
    pub fn normalize_s(self) -> Self {
        let neg_s = self.s.neg();
        if neg_s < self.s { EcdsaSignature { s: neg_s, ..self } } else { self }
    }

    /// Generates a set of ECDSA signature shares from the signature `s` value.
    ///
    /// This function takes an ECDSA signature and divides the `s` component into
    /// `n` distinct shares. Each share is represented as an [`EcdsaSignatureShare`]
    /// and can be used for threshold signing or distributed signature schemes.
    pub fn generate_shares(&self, n: u16) -> Result<Vec<EcdsaSignatureShare>, EcdsaSignatureError> {
        let mut csprng = OsRng;

        let EcdsaSignature { r, s } = self;

        let mut sig_shares: Vec<Scalar<Secp256k1>> =
            (0..n.saturating_sub(1)).map(|_| Scalar::<Secp256k1>::random(&mut csprng)).collect();

        // Computes last share according to random shares and signature s value
        let add_shares =
            sig_shares.iter().cloned().reduce(|acc, x| acc.add(x)).ok_or(EcdsaSignatureError::AccumulateShares)?;
        let last_share = s.sub(add_shares);
        sig_shares.push(last_share);

        // Collect sigmas into signature share type
        let signature_shares =
            sig_shares.into_iter().map(|sigma| EcdsaSignatureShare { r: r.into_inner(), sigma }).collect();
        Ok(signature_shares)
    }
}

impl fmt::Display for EcdsaSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EcdsaSignature {{ r: {:?}, s: {:?} }}", self.r, self.s)
    }
}

/// A struct representing an ECDSA private key.
/// The private key is a non-zero scalar defined on the secp256k1 elliptic curve.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EcdsaSignatureShare {
    /// r component of signature share
    pub r: Scalar<Secp256k1>,
    /// sigma component of partial signature share
    pub sigma: Scalar<Secp256k1>,
}

impl EcdsaSignatureShare {
    /// Recovers signatures shares into regular signature.
    ///
    /// Returns `None` if input is malformed.
    ///
    /// `recover` may return a signature that's invalid for public key and message it was issued for.
    /// This would mean that some of signers cheated and aborted the protocol. You need to validate
    /// resulting signature to be sure that no one aborted the protocol.
    pub fn recover(signature_shares: &[EcdsaSignatureShare]) -> Option<EcdsaSignature> {
        // Take the first and ensure there are signature shares to process
        let first_share = signature_shares.first()?;
        // Extract `r` from the first share
        let r = NonZero::from_scalar(first_share.r)?;
        // Sum `sigma` values
        let s_total = NonZero::from_scalar(signature_shares.iter().map(|s| s.sigma).sum())?;
        // Return a normalized EcdsaSignature
        Some(EcdsaSignature { r, s: s_total }.normalize_s())
    }
}

/// Enum representing errors that can occur when handling EdDSA signature.
#[derive(Error, Debug)]
pub enum EddsaSignatureError {
    /// Error when a signature component is invalid
    #[error("Invalid signature component: {0}")]
    InvalidComponentSignature(String),
}

/// Enum representing errors that can occur when handling ECDSA signature.
#[derive(Error, Debug)]
pub enum EcdsaSignatureError {
    /// Error during accumulation process for ecdsa signature generation.
    #[error("Error during accumulation process for ecdsa signature generation.")]
    AccumulateShares,
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{privatekey::ThresholdPrivateKey, publickey::ThresholdPublicKey};
    use cggmp21::signing::{DataToSign, Signature as Cggmp21Signature};
    use generic_ec::{
        coords::AlwaysHasAffineX,
        curves::{Ed25519, Secp256k1},
        NonZero, Point, SecretScalar,
    };
    use givre::ciphersuite::{Ed25519 as Ed25519Ciphersuite, NormalizedPoint};
    use sha2::Sha256;

    fn generate_signature_and_shares_test(
        n: u16,
    ) -> (DataToSign<Secp256k1>, ThresholdPrivateKey<Secp256k1>, EcdsaSignature, Vec<EcdsaSignatureShare>) {
        // 1. Message generation
        let message_to_sign = b"Transaction with plenty of bitcoin";
        let message_digest = DataToSign::digest::<Sha256>(message_to_sign);

        // 2. Secret key generation
        let mut csprng = OsRng;
        let sk_val = SecretScalar::<Secp256k1>::random(&mut csprng);

        // 3. Generate signature:
        // random scalar k
        let k = Scalar::<Secp256k1>::random(&mut csprng);
        // random point R = k * G
        let r_point = Point::<Secp256k1>::generator().to_point() * &k;
        // r = R.x()
        let r_point = NonZero::from_point(r_point).unwrap();
        let r = r_point.x().to_scalar();
        let r = NonZero::from_scalar(r).unwrap();
        // s = k.invert() * (h + r * sk)
        let s = (message_digest.to_scalar() + r * sk_val.clone()) * k.invert().unwrap();
        let s = NonZero::from_scalar(s).unwrap();
        let signature = EcdsaSignature { r, s }.normalize_s();
        let sk: ThresholdPrivateKey<Secp256k1> = ThresholdPrivateKey::<Secp256k1>::from_scalar(sk_val).unwrap();

        // 4. Generate shares of signature
        let signature_shares = signature.generate_shares(n).unwrap();

        (message_digest, sk, signature, signature_shares)
    }

    fn verify(pk: ThresholdPublicKey<Secp256k1>, signature: EcdsaSignature, message: &DataToSign<Secp256k1>) -> bool {
        let EcdsaSignature { r, s } = signature;
        let cggmp_sig = Cggmp21Signature { r, s };

        let pk = pk.as_point();
        cggmp_sig.verify(pk, message).is_ok()
    }

    #[test]
    fn test_combine_and_verify() {
        let (msg_dig, sk, sig, sig_shares) = generate_signature_and_shares_test(5);
        let sig_reconstructed = EcdsaSignatureShare::recover(&sig_shares).unwrap();
        assert_eq!(sig_reconstructed, sig);

        let pk = ThresholdPublicKey::<Secp256k1>::from_private_key(&sk);

        let verifies = verify(pk, sig_reconstructed, &msg_dig);

        assert!(verifies)
    }

    #[test]
    fn test_from_components_bytes() {
        // Normalized r point and z scalar
        let k = Scalar::<Ed25519>::random(&mut OsRng);
        let r_point = Point::<Ed25519>::generator().to_point() * &k;
        let r = NormalizedPoint::<Ed25519Ciphersuite, Point<Ed25519>>::try_normalize(r_point)
            .expect("Failed to normalize point");
        let z = Scalar::<Ed25519>::random(&mut OsRng);

        // Convert to bytes
        let r_bytes = r.to_bytes();
        let z_bytes = z.to_le_bytes();

        let signature = EddsaSignature::from_components_bytes(&r_bytes, &z_bytes.as_ref())
            .expect("Failed to create signature from components");

        // Verify signature components match the originals
        assert_eq!(signature.signature.r.to_bytes(), r_bytes);
        assert_eq!(signature.signature.z.to_le_bytes(), z_bytes);
    }
}
