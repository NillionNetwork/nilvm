//! Program definitions.

use anyhow::{anyhow, Error};
use mpc_vm::{protocols::MPCProtocol, JitCompiler, MPCCompiler, Program, ProgramBytecode};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

/// A program.
#[derive(Clone, Debug)]
pub struct ProgramMetadata {
    /// The raw mir bytes.
    pub raw_mir: Vec<u8>,
}

impl ProgramMetadata {
    /// Get the mir for this program.
    pub fn mir(&self) -> Result<ProgramMIR, Error> {
        let mir = ProgramMIR::try_decode(&self.raw_mir)?;
        Ok(mir)
    }

    /// Get the MIR in raw form.
    pub fn raw_mir(&self) -> Vec<u8> {
        self.raw_mir.clone()
    }
}

/// The programs defined in a package.
#[derive(Clone, Debug)]
pub struct PackagePrograms {
    /// The metadata for every program in this package.
    pub metadata: HashMap<String, ProgramMetadata>,
    #[allow(clippy::type_complexity)]
    compiled_programs: Arc<Mutex<HashMap<String, (Program<MPCProtocol>, ProgramBytecode)>>>,
}

impl PackagePrograms {
    /// Get the metadata for the program with the given name.
    pub fn metadata(&self, name: &str) -> Option<&ProgramMetadata> {
        self.metadata.get(name)
    }

    /// Get the mir for the given program name.
    pub fn mir(&self, name: &str) -> Result<ProgramMIR, Error> {
        let program = self.metadata(name).ok_or_else(|| anyhow!("program not found {}", name))?;
        program.mir()
    }

    /// Compile the program with the given name.
    ///
    /// This uses a cache internally so every program is only compiled once.
    pub fn program(&self, program_name: &str) -> Result<(Program<MPCProtocol>, ProgramBytecode), Error> {
        let mut cache = self.compiled_programs.lock().unwrap();
        if let Some(program) = cache.get(program_name) {
            return Ok(program.clone());
        }
        let mir = self.mir(program_name)?;
        let program = MPCCompiler::compile_with_bytecode(mir)?;
        cache.insert(program_name.to_string(), program.clone());
        Ok(program)
    }

    /// Gets the list of all the program names.
    pub fn program_names(&self) -> impl Iterator<Item = &String> {
        self.metadata.keys()
    }
}

impl<T> From<T> for PackagePrograms
where
    HashMap<String, ProgramMetadata>: From<T>,
{
    fn from(programs: T) -> Self {
        let programs = HashMap::from(programs);
        Self { metadata: programs, compiled_programs: Default::default() }
    }
}
