use program_builder::compile::run_on_directory;
use std::{env, path::PathBuf};

fn compile_protos() {
    env::set_var("PROTOC", protobuf_src::protoc());
    let protos = [
        // programs
        "proto/node/programs/v1/programs.proto",
        // values
        "proto/node/values/v1/values.proto",
        // results
        "proto/node/compute/v1/result.proto",
    ];
    tonic_build::configure()
        .protoc_arg("--fatal_warnings")
        .extern_path(".nillion.permissions.v1", "node_api::permissions::proto")
        .extern_path(".nillion.auth.v1", "node_api::auth::proto")
        .extern_path(".nillion.values.v1", "node_api::values::proto")
        .extern_path(".nillion.membership.v1", "node_api::membership::proto")
        .compile_protos(&protos, &["./proto", "../libs/node-api/proto"])
        .expect("compilation failed");
}

fn compile_builtin_programs() {
    let nada_dsl_path = env::current_dir().unwrap().join("../nada-lang/nada_dsl");
    let programs_directories = vec![PathBuf::from("builtin-programs")];
    if let Err(e) = run_on_directory("builtin", programs_directories, &nada_dsl_path) {
        println!("cargo::warning=failed to compile built in programs: {e:#}");
        panic!("Failed to compile builtin programs");
    }
}

fn main() {
    compile_protos();
    compile_builtin_programs();
}
