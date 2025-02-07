//! Compilation module for automated tests

use std::{
    path::{Path, PathBuf},
    sync::mpsc::channel,
    thread,
    time::Duration,
};

use anyhow::{anyhow, Error};
use log::debug;
use pynadac::{Compiler, CompilerOptions, PersistOptions};

use crate::runner::TestCase;

/// Compilation Timeout in ms
const TIMEOUT: u64 = 5000;

/// Compile a program test case
fn compile_test_case(test_case: TestCase, tmp_dir: PathBuf) -> Result<TestCase, Error> {
    let options = CompilerOptions { persist: PersistOptions { mir_bin: true, mir_json: false } };
    let mut test_case = test_case;
    let compiler = Compiler::with_options(tmp_dir, options);
    debug!("Compiling {} : {}", test_case.name, test_case.program_path.to_string_lossy());
    test_case.compile_output = Some(compiler.compile(&test_case.program_path.to_string_lossy())?);
    debug!("Compiled {} : {}", test_case.name, test_case.program_path.to_string_lossy());
    Ok(test_case)
}

pub fn compile_with_timeout(test_case: &TestCase, tmp_dir: &Path) -> Result<TestCase, Error> {
    let (sender, receiver) = channel();
    let program_path = test_case.program_path.clone();
    let test_case = TestCase {
        program_path: test_case.program_path.clone(),
        name: test_case.name.clone(),
        compile_output: None,
        inputs: test_case.inputs.clone(),
    };
    let tmp_dir = tmp_dir.to_path_buf();
    thread::spawn(move || {
        sender
            .send(compile_test_case(
                TestCase {
                    program_path: test_case.program_path,
                    name: test_case.name,
                    compile_output: None,
                    inputs: test_case.inputs,
                },
                tmp_dir.clone(),
            ))
            .unwrap();
    });

    match receiver.recv_timeout(Duration::from_millis(TIMEOUT)) {
        Ok(result) => result,
        Err(_) => Err(anyhow!("Compilation timed out for {}", program_path.to_string_lossy())),
    }
}
