//! Common errors

use thiserror::Error;

/// Unimplemented feature error
#[derive(Error, Debug, Clone, PartialEq)]
#[error("{0} is unimplemented")]
pub struct UnimplementedError(pub String);

impl From<&str> for UnimplementedError {
    fn from(s: &str) -> Self {
        UnimplementedError(s.to_owned())
    }
}

impl From<String> for UnimplementedError {
    fn from(s: String) -> Self {
        UnimplementedError(s)
    }
}
