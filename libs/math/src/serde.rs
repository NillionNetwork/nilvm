//! Serde abstractions.

/// A trait to abstract conditional serde requirements.
#[cfg(feature = "serde")]
pub trait Serde: serde::Serialize + serde::de::DeserializeOwned {}

#[cfg(feature = "serde")]
impl<T: serde::Serialize + serde::de::DeserializeOwned> Serde for T {}

/// A trait to abstract conditional serde requirements.
#[cfg(not(feature = "serde"))]
pub trait Serde {}

// If serde is disabled then technically any type is Serde
#[cfg(not(feature = "serde"))]
impl<T> Serde for T {}
