//! Nada python frontend.

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

mod compile;
mod eval;

pub use compile::{CompileOutput, Compiler, CompilerOptions, PersistOptions};
use std::process::Command;
use thiserror::Error;

/// The nada-dsl minor version supported.
///
///
/// nada-dsl releases follow semantic version format: x.y.z,
/// where the first digit is the major version, the second is the minor version
/// and the third is the patch version.
///
/// pynadac is compatible with all the nada-dsl releases
/// whose major and minor version is specified by [`NADA_DSL_VERSION`].
///
/// Example:
/// If [`NADA_DSL_VERSION`] is 0.1, the nada-dsl supported versions are
/// the ones whose major version is 0 and minor version is 1. For instance:
/// - 0.1.10 is supported
/// - 0.1.0 is supported
/// - 0.2.1 is not supported because the minor version doesn't match.
pub const NADA_DSL_VERSION: &str = "0.8";

/// The `pip` command.
pub const PIP: &str = "pip";

pub fn parse_dsl_version(pip_show_output: &str) -> &str {
    for line in pip_show_output.lines() {
        if line.starts_with("Version:") {
            return match line.split_once(' ') {
                Some((_, version)) => version,
                None => "",
            };
        }
    }
    ""
}

#[derive(Clone, Debug, Error)]
pub enum CheckVersionError {
    #[error("missing nada-dsl version")]
    MissingVersion,

    #[error("The installed nada-dsl version {0} is incompatible. This release supports nada-dsl versions {1}.*")]
    IncompatibleVersion(String, String),

    #[error("pip show command returned invalid output")]
    InvalidPipShowOutput,
}

/// Checks that pynadac version matches the DSL version
///
/// It uses `pip show` which is a reasonable expectation that
/// users will have installed giving that they need to Python packages.
pub fn check_version_matches() -> Result<(), CheckVersionError> {
    // Let's check that the DSL actually provides a version
    match Command::new(PIP).args(["show", "nada_dsl"]).output() {
        Ok(output) => {
            let stdout = String::from_utf8(output.stdout).map_err(|_| CheckVersionError::InvalidPipShowOutput)?;

            let dsl_version = parse_dsl_version(&stdout);
            if dsl_version.is_empty() {
                return Err(CheckVersionError::MissingVersion);
            } else if !dsl_version.starts_with(NADA_DSL_VERSION) {
                return Err(CheckVersionError::IncompatibleVersion(
                    dsl_version.to_string(),
                    NADA_DSL_VERSION.to_string(),
                ));
            }
        }
        Err(_) => {
            return Err(CheckVersionError::MissingVersion);
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{check_version_matches, parse_dsl_version, CheckVersionError, NADA_DSL_VERSION};

    fn pip_show_output() -> String {
        format!(
            r#"Name: nada_dsl
Version: {NADA_DSL_VERSION}.3
Summary: Nillion Nada DSL to create Nillion MPC programs.
Home-page: 
Author: 
Author-email: 
Location: /Users/juan/dev/nada_dsl/venv/lib/python3.10/site-packages
Requires: asttokens, parsial, richreports, sortedcontainers
Required-by: 
"#
        )
    }

    #[test]
    fn test_parse_dsl_version() {
        assert_eq!(format!("{NADA_DSL_VERSION}.3"), parse_dsl_version(&pip_show_output()))
    }

    #[test]
    fn test_compatible_nada_dsl_version() -> Result<(), CheckVersionError> {
        check_version_matches()
    }
}
