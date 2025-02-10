use once_cell::sync::Lazy;
use program_builder::{program_package, PackagePrograms};

pub static PROGRAMS: Lazy<PackagePrograms> = Lazy::new(|| program_package!("default"));
