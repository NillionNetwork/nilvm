//! Nada python frontend.

use anyhow::{anyhow, bail, Context, Result};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use crate::eval::{EvalOutput, Evaluator};
use nada_compiler_backend::{
    mir::{
        proto::{ConvertProto, Message},
        ProgramMIR, MIR_FILE_EXTENSION_BIN, MIR_FILE_EXTENSION_JSON,
    },
    preprocess::preprocess,
    validators::{report::ValidationContext, Validator},
};

use serde_files_utils::json::write_json;

/// The output persistence options.
#[derive(Clone, Default)]
pub struct PersistOptions {
    /// Whether to persist the MIR BIN into a file.
    pub mir_bin: bool,

    /// Whether to persist the MIR JSON into a file.
    pub mir_json: bool,
}

/// The compiler options.
#[derive(Clone, Default)]
pub struct CompilerOptions {
    /// Options related to program persistence.
    pub persist: PersistOptions,
}

/// A nada compiler for python programs.
pub struct Compiler {
    target_dir: PathBuf,
    options: CompilerOptions,
}

impl Compiler {
    /// Create a new compiler that will generate outputs in the given directory.
    ///
    /// This will use the default options, meaning it will not generate any extra files.
    pub fn new<T: Into<PathBuf>>(target_dir: T) -> Self {
        let target_dir = target_dir.into();
        let options = CompilerOptions::default();
        Self { target_dir, options }
    }

    /// Create a new compiler using the provided options.
    pub fn with_options<T: Into<PathBuf>>(target_dir: T, options: CompilerOptions) -> Self {
        let target_dir = target_dir.into();
        Self { target_dir, options }
    }

    pub fn eval_program<P: AsRef<Path>>(program_path: P) -> Result<EvalOutput> {
        let evaluator = Evaluator::new(program_path)?;
        evaluator.eval()
    }

    fn eval_program_str(program_str: &str) -> Result<EvalOutput> {
        Evaluator::eval_str(program_str)
    }

    /// Compile the python program in the given path with the given name.
    pub fn compile_with_name(&self, program_path: &str, program_name: &str) -> Result<CompileOutput> {
        let EvalOutput { mir } = Self::eval_program(program_path)?;
        let mir = preprocess(mir)?;
        let validation_result = mir.validate()?;

        let mir_json_file = self.persist_mir_json(program_name, &mir)?;
        let mir_bin_file = self.persist_mir_bin(program_name, &mir)?;
        let output = CompileOutput {
            mir,
            program_name: program_name.to_string(),
            mir_bin_file,
            mir_json_file,
            validation_result,
        };
        Ok(output)
    }

    /// Compile the python program in the given path.
    pub fn compile(&self, program_path: &str) -> Result<CompileOutput> {
        let program_name = parse_program_name(program_path)
            .with_context(|| format!("failed to parse program name from path: {program_path}"))?;
        self.compile_with_name(program_path, &program_name)
    }

    /// Compile the python program in the given string.
    pub fn compile_str(program_str: &str, program_name: &str) -> Result<CompileOutput> {
        let EvalOutput { mir } = Self::eval_program_str(program_str)?;
        let mir = preprocess(mir)?;
        let validation_result = mir.validate()?;

        let output = CompileOutput {
            mir,
            program_name: program_name.to_string(),
            mir_bin_file: None,
            mir_json_file: None,
            validation_result,
        };
        Ok(output)
    }

    fn persist_mir_bin(&self, program_name: &str, mir: &ProgramMIR) -> Result<Option<PathBuf>> {
        if self.options.persist.mir_bin {
            let output_path = self.build_file_path(program_name, MIR_FILE_EXTENSION_BIN);
            let proto_mir = mir.clone().into_proto();
            let mut buf = Vec::new();
            proto_mir.encode(&mut buf)?;
            File::create(&output_path)
                .with_context(|| format!("failed to create file: {}", output_path.to_string_lossy()))?
                .write_all(&buf)
                .with_context(|| format!("failed to write to file: {}", output_path.to_string_lossy()))?;
            Ok(Some(output_path))
        } else {
            Ok(None)
        }
    }

    fn persist_mir_json(&self, program_name: &str, mir: &ProgramMIR) -> Result<Option<PathBuf>> {
        if self.options.persist.mir_json {
            let output_path = self.build_file_path(program_name, MIR_FILE_EXTENSION_JSON);
            write_json(&output_path, mir)?;
            Ok(Some(output_path))
        } else {
            Ok(None)
        }
    }

    fn build_file_path(&self, file_name: &str, extension: &str) -> PathBuf {
        let file_name = format!("{file_name}{extension}");
        self.target_dir.join(file_name)
    }
}

/// The compiler's output.
pub struct CompileOutput {
    /// The program name.
    pub program_name: String,

    /// The output MIR.
    pub mir: ProgramMIR,

    /// The path to the MIR binary output file, if any.
    pub mir_bin_file: Option<PathBuf>,

    /// The path to the MIR JSON output file, if any.
    pub mir_json_file: Option<PathBuf>,

    /// The MIR validation result
    pub validation_result: ValidationContext,
}

fn parse_program_name(path: &str) -> Result<String> {
    let (base, extension) = path.rsplit_once('.').ok_or_else(|| anyhow!("file has no extension"))?;
    if extension != "py" {
        bail!("expected .py file extension");
    }
    let program_name = match base.rsplit_once('/') {
        Some((_, name)) => name,
        None => base,
    };
    Ok(program_name.into())
}

#[cfg(test)]
mod tests {
    use crate::compile::Compiler;

    #[test]
    fn test_compile_str() {
        let program_str = r#"
from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    my_int1 = SecretInteger(Input(name="my_int1", party=party1))
    my_int2 = SecretInteger(Input(name="my_int2", party=party1))
    my_int3 = SecretInteger(Input(name="my_int3", party=party1))
    my_int4 = SecretInteger(Input(name="my_int4", party=party1))

    new_int1 = my_int1 * my_int2
    new_int2 = my_int3 * my_int4
    new_int3 = new_int1 + new_int2

    return [Output(new_int3, "my_output", party1)]
    "#;
        Compiler::compile_str(program_str, "test_program").unwrap();
    }

    #[test]
    fn test_compile_boolean_or() {
        let program_str = r#"
from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    A = SecretInteger(Input(name="A_neg", party=party1))
    B = SecretInteger(Input(name="B_neg", party=party2))
    C = SecretInteger(Input(name="C", party=party2))

    result = (A < (B + C)) | (A < C)

    return [Output(result, "my_output", party1)]
        "#;

        Compiler::compile_str(program_str, "test_program").unwrap();
    }
}
