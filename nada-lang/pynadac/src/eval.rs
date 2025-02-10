//! Program evaluation.

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose, Engine};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use serde::{Deserialize, Serialize};
use std::{env, fs::File, path::Path, process::Command};

pub const PYTHON: &str = "python";

pub(crate) struct Evaluator {
    program_path: String,
}

impl Evaluator {
    pub(crate) fn new<P: AsRef<Path>>(program_path: P) -> Result<Self> {
        let Some(program_path) = program_path.as_ref().to_str() else { bail!("invalid program path") };
        Ok(Self { program_path: program_path.to_string() })
    }

    pub(crate) fn eval(self) -> Result<EvalOutput> {
        Self::validate_python_exists()?;
        if File::open(&self.program_path).is_err() {
            bail!("failed to open source file: {}", &self.program_path);
        }
        let output = Self::invoke_script(&["-m", "nada_dsl.compile", &self.program_path])?;
        match output {
            ScriptOutput::Success { mir } => {
                let mir = ProgramMIR::try_decode(&mir).context("failed parsing program MIR")?;

                Ok(EvalOutput { mir })
            }
            ScriptOutput::Failure { reason: _, traceback } => bail!("{traceback}"),
        }
    }

    /// Evaluate a program in string format
    pub(crate) fn eval_str(program_str: &str) -> Result<EvalOutput> {
        Self::validate_python_exists()?;
        let base64_encoded = general_purpose::STANDARD.encode(program_str);
        let output = Self::invoke_script(&["-m", "nada_dsl.compile", "-s", &base64_encoded])?;
        match output {
            ScriptOutput::Success { mir } => {
                let mir = ProgramMIR::try_decode(&mir).context("failed parsing program MIR")?;
                Ok(EvalOutput { mir })
            }
            ScriptOutput::Failure { reason: _, traceback } => bail!("{traceback}"),
        }
    }

    /// Invokes a script using Python
    fn invoke_script(args: &[&str]) -> Result<ScriptOutput> {
        let output = Command::new(PYTHON).args(args).output()?;
        let output = serde_json::from_slice(&output.stdout).with_context(|| {
            format!(
                "invalid output: stderr:{} stdout:{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            )
        })?;
        Ok(output)
    }

    fn validate_python_exists() -> Result<()> {
        let path_env = env::var_os("PATH").ok_or_else(|| anyhow!("PATH environment variable not set"))?;
        for path in env::split_paths(&path_env) {
            let full_path = path.join(PYTHON);
            if full_path.is_file() {
                return Ok(());
            }
        }
        bail!("python binary not found")
    }
}

pub struct EvalOutput {
    pub mir: ProgramMIR,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result")]
enum ScriptOutput {
    Success { mir: Vec<u8> },
    Failure { reason: String, traceback: String },
}
