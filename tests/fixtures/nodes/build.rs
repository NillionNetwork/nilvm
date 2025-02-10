use program_builder::compile::run_on_directory;
use std::path::PathBuf;

fn main() {
    let nada_dsl_path = std::env::current_dir().unwrap().join("../../../nada-lang/nada_dsl");
    let programs_directories = vec![PathBuf::from("programs")];
    run_on_directory("default", programs_directories, &nada_dsl_path).expect("compilation failed");
}
