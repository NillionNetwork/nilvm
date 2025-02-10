//! Utilities for tools that use the clap crate.

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

#[cfg(feature = "shell-completions")]
pub mod shell_completions;

use build_info::BuildInfo;
use clap::{CommandFactory, FromArgMatches, Parser};

/// An extension trait for [clap::Parser].
pub trait ParserExt: Parser {
    /// Parse the command using the version pulled via [build_info::BuildInfo].
    fn parse_with_version() -> Self;
}

// This is copied from `clap::Parser` with the only addition of pulling the `release_version` and feed
// it into version.
impl<T: Parser> ParserExt for T {
    fn parse_with_version() -> Self {
        let info = BuildInfo::default();

        // Determine short version.
        let git_commit_hash = info.git_commit_hash;
        let release_version = info.release_version;
        let version = release_version.unwrap_or(git_commit_hash);

        // Build long version.
        //
        // If built with release pipeline variables, then --version reports, e.g.:
        //
        // $ nada-run --version
        // nada-run v0.1.0
        // Release candidate version: v0.1.0-rc.1
        // Git commit hash: f79db5f87fd4527c096ba95c5b536d3634cc0aa0
        //
        // Otherwise:
        //
        // $ nada-run --version
        // nada-run f79db5f87fd4527c096ba95c5b536d3634cc0aa0
        let mut long_version = String::from(version);

        if let (Some(_), Some(release_candidate_version)) = (release_version, info.release_candidate_version) {
            long_version.push_str(&format!("\nRelease candidate version: {release_candidate_version}\n"));
            long_version.push_str(&format!("Git commit hash: {git_commit_hash}"));
        }

        let mut matches = <Self as CommandFactory>::command().version(version).long_version(long_version).get_matches();
        let res = <Self as FromArgMatches>::from_arg_matches_mut(&mut matches).map_err(format_error::<Self>);
        match res {
            Ok(s) => s,
            Err(e) => {
                // Since this is more of a development-time error, we aren't doing as fancy of a quit
                // as `get_matches`
                e.exit()
            }
        }
    }
}

fn format_error<I: CommandFactory>(err: clap::Error) -> clap::Error {
    let mut cmd = I::command();
    err.format(&mut cmd)
}
