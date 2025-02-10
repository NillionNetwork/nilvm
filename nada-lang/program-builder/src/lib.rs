//! Builds nada programs as part of a crate's build process.

#![deny(missing_docs)]

#[cfg(feature = "compile")]
pub mod compile;

pub mod program;
pub use program::{PackagePrograms, ProgramMetadata};

/// Includes the definition of the program package with the given name.
///
/// This is meant to be used in crate code once `run_on_directory` has been run for the same
/// package name. This expression returns a [`PackagePrograms`] that contains all the programs in
/// this package.
#[macro_export]
macro_rules! program_package {
    ($package: tt) => {
        include!(concat!(env!("OUT_DIR"), concat!("/", $package, "/programs.rs")));
    };
}
