//! This crate allows getting information about the environment it was built on.
//!
//! Crates that want to know what git hash, architecture, etc. the build used should import this
//! crate and use [BuildInfo::default] to access that.
//!
//! # Examples
//!
//! ```rust
//! # use build_info::BuildInfo;
//! let info = BuildInfo::default();
//! println!("The git hash is {}", info.git_commit_hash);
//! ```

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

use std::{borrow::Cow, env};

use serde::{Deserialize, Serialize};

/// A copy-on-write string.
pub type CowString = Cow<'static, str>;

/// Information about the build.
///
/// Use [BuildInfo::default] to access information about the environment this crate was built with.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BuildInfo {
    /// The git commit hash the build used.
    pub git_commit_hash: &'static str,

    /// The architecture the crate was built on.
    pub arch: &'static str,

    /// The OS the crate was built on.
    pub os: &'static str,

    /// The rustc version the crate was built with.
    pub rustc_version: &'static str,

    /// The time at which this crate was built.
    pub build_timestamp: u64,

    /// The release candidate version, e.g. v0.1.0-rc.1.
    pub release_candidate_version: Option<&'static str>,

    /// The release version without the release candidate part, e.g. v0.1.0-rc.1 -> v0.1.0.
    pub release_version: Option<&'static str>,
}

impl Default for BuildInfo {
    fn default() -> Self {
        BuildInfo {
            git_commit_hash: env!("NILLION_GIT_COMMIT_HASH"),
            arch: env::consts::ARCH,
            os: env::consts::OS,
            rustc_version: env!("NILLION_RUSTC_VERSION"),
            // SAFETY: this is guaranteed to be a number because we generate it in build.rs.
            #[allow(clippy::unwrap_used)]
            build_timestamp: env!("NILLION_BUILD_TIMESTAMP").parse().unwrap(),
            release_candidate_version: option_env!("NILLION_RELEASE_CANDIDATE_VERSION"),
            release_version: option_env!("NILLION_RELEASE_VERSION"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_info() {
        let info = BuildInfo::default();
        assert_eq!(info.git_commit_hash.len(), 40);
        assert!(!info.arch.is_empty());
        assert!(!info.os.is_empty());
        assert!(!info.rustc_version.is_empty());
        // At least after July 1st 2023.
        assert!(info.build_timestamp > 1688169600);
    }
}
