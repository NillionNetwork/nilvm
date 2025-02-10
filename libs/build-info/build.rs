use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn run_command<const N: usize>(command: &str, args: [&str; N]) -> String {
    let output = Command::new(command).args(args).output().expect("failed to get run command");
    String::from_utf8(output.stdout).expect("invalid command output")
}

fn git_hash() -> String {
    run_command("git", ["rev-parse", "HEAD"])
}

fn rustc_version() -> String {
    run_command("rustc", ["--version"])
}

fn main() {
    let hash = git_hash();
    let rustc_version = rustc_version();
    let unix_now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    println!("cargo:rustc-env=NILLION_GIT_COMMIT_HASH={hash}");
    println!("cargo:rustc-env=NILLION_RUSTC_VERSION={rustc_version}");
    println!("cargo:rustc-env=NILLION_BUILD_TIMESTAMP={unix_now}");
    println!("cargo:rustc-rerun-if-changed=../../.git/HEAD");
}
