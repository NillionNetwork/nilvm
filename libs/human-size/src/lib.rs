//! This crate implements the human size utilities

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]
#![allow(clippy::module_inception)]

use std::{
    fmt::{Display, Formatter},
    num::ParseIntError,
    str::FromStr,
};

/// A newtype that allows parsing a usize from string representations that contain units like "K"
/// for 1000 and "M" for 1000000
#[derive(Default, Clone, Debug)]
pub struct HumanSize(pub usize);

impl Display for HumanSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HumanSize {{ {} }}", self.0)
    }
}

/// This errors are thrown during a human size type is parsed
#[derive(thiserror::Error, Debug)]
pub enum FromStrError {
    /// Parse error
    #[error("regex parse failed")]
    ParseError(#[from] regex::Error),
    /// String doesn't match with the string
    #[error("string doesn't matched")]
    NotMatch,
    /// Human size string doesn't content digits
    #[error("digits not found")]
    DigitsNotFound,
    /// Usize parse has failed
    #[error("usize parse failed")]
    UsizeParseFailed(#[from] ParseIntError),
    /// Invalid format
    #[error("invalid format")]
    InvalidFormat,
}

impl FromStr for HumanSize {
    type Err = FromStrError;

    #[allow(clippy::arithmetic_side_effects)]
    fn from_str(v: &str) -> Result<Self, Self::Err> {
        let regex = regex::Regex::new(r"^(\d+)\s*(M|K)?$")?;
        let cap = regex.captures(v).take().ok_or(Self::Err::NotMatch)?;
        let value = cap.get(1).take().ok_or(Self::Err::DigitsNotFound)?;
        let value = value.as_str().parse::<usize>()?;
        match cap.get(2).map_or("", |m| m.as_str()) {
            "M" => Ok(HumanSize(value * 1000000)),
            "K" => Ok(HumanSize(value * 1000)),
            "" => Ok(HumanSize(value)),
            _ => Err(Self::Err::InvalidFormat),
        }
    }
}

pub mod serde {
    //! Human size serde utilities

    use crate::{FromStrError, HumanSize};
    use serde::{de::Error, Deserialize, Deserializer};
    use std::str::FromStr;

    impl<'de> Deserialize<'de> for HumanSize {
        /// Deserialize string matches with "^(\d+)\s*(M|K)$"
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            FromStr::from_str(&s).map_err(|err: FromStrError| Error::custom(err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{FromStrError, HumanSize};
    use std::str::FromStr;

    #[test]
    fn test_100() -> Result<(), FromStrError> {
        let str = "100";
        let size: HumanSize = FromStr::from_str(str)?;
        assert_eq!(100, size.0);
        Ok(())
    }

    #[test]
    fn test_100k() -> Result<(), FromStrError> {
        let str = "100K";
        let size: HumanSize = FromStr::from_str(str)?;
        assert_eq!(100000, size.0);
        Ok(())
    }

    #[test]
    fn test_10m() -> Result<(), FromStrError> {
        let str = "10M";
        let size: HumanSize = FromStr::from_str(str)?;
        assert_eq!(10000000, size.0);
        Ok(())
    }

    #[test]
    #[should_panic]
    #[allow(clippy::unwrap_used)]
    fn test_10b() {
        let str = "10B";
        let _: HumanSize = FromStr::from_str(str).unwrap();
    }
}
