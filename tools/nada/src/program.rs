use crate::{
    build,
    nada_project_toml::{NadaProjectToml, ProgramConf},
    paths::{get_compiled_program_path, get_target_path},
};
use eyre::{eyre, Result};
use math_lib::modular::U64SafePrime;
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{plan::ExecutionPlan, ExecutionVmConfig},
    JitCompiler, MPCCompiler, Program as JitProgram, ProgramBytecode,
};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use std::{
    collections::HashMap,
    fs,
    fs::File,
    io::Read,
    sync::{Arc, LazyLock, Mutex},
};

static BUILT_PROGRAMS: LazyLock<Mutex<HashMap<String, Arc<Program>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn get_program(mir: ProgramMIR) -> Result<(JitProgram<MPCProtocol>, ProgramBytecode)> {
    MPCCompiler::compile_with_bytecode(mir).map_err(|e| eyre!("failed to compile program's MIR: {e}"))
}

pub fn get_program_mir(program_conf: &ProgramConf) -> Result<ProgramMIR> {
    let program_path = get_compiled_program_path(program_conf)?;
    let mut program = vec![];
    File::open(&program_path)
        .map_err(|e| eyre!("failed to open program's MIR file: {e}"))?
        .read_to_end(&mut program)
        .map_err(|e| eyre!("failed to read program's MIR file: {e}"))?;
    let program_mir = ProgramMIR::try_decode(&program).map_err(|e| eyre!("failed to parse program's MIR: {e}"))?;
    Ok(program_mir)
}

pub struct Program {
    pub conf: ProgramConf,
    pub mir: ProgramMIR,
    pub bytecode: ProgramBytecode,
    pub program: JitProgram<MPCProtocol>,
}

impl Program {
    pub fn build(conf: &NadaProjectToml, program_name: &String) -> Result<Arc<Program>> {
        let mut built_programs = BUILT_PROGRAMS.lock().map_err(|_| eyre!("Built programs lock poisoned"))?;
        if let Some(program) = built_programs.get(program_name).cloned() {
            return Ok(program);
        }
        let programs = conf.get_programs()?;
        let program_conf = programs.get(program_name).ok_or_else(|| eyre!("Program not found: {program_name}"))?;
        build::build_program(program_conf, false)?;
        let mir = get_program_mir(program_conf)?;
        let (program, bytecode) = get_program(mir.clone())?;
        let program = Arc::new(Program { conf: program_conf.clone(), mir, bytecode, program });
        built_programs.insert(program_name.clone(), program.clone());
        Ok(program)
    }

    pub fn mir_to_text(&self) -> Result<()> {
        let target_path = get_target_path()?;
        let file_path = target_path.join(format!("{}.nada.mir.txt", self.conf.name));
        let bytecode_txt = self.mir.text_repr();
        fs::write(file_path, bytecode_txt)?;
        Ok(())
    }

    pub fn bytecode_to_json(&self) -> Result<()> {
        let target_path = get_target_path()?;
        let contract_path = target_path.join(format!("{}.nada.contract.json", self.conf.name));
        let contract_json = serde_json::to_string_pretty(&self.program.contract)?;
        fs::write(contract_path, contract_json)?;
        let bytecode_path = target_path.join(format!("{}.nada.bytecode.json", self.conf.name));
        let bytecode_json = serde_json::to_string_pretty(&self.bytecode)?;
        fs::write(bytecode_path, bytecode_json)?;
        Ok(())
    }

    pub fn bytecode_to_text(&self) -> Result<()> {
        let target_path = get_target_path()?;
        let file_path = target_path.join(format!("{}.nada.bytecode.txt", self.conf.name));
        let bytecode_txt = self.bytecode.text_repr();
        fs::write(file_path, bytecode_txt)?;
        Ok(())
    }

    pub fn protocols_to_text(&self, execution_vm_config: &ExecutionVmConfig) -> Result<()> {
        let target_path = get_target_path()?;
        let file_path = target_path.join(format!("{}.nada.protocols.txt", self.conf.name));
        let mut protocols_txt = self.program.body.contract_text_repr();
        // This plan is created only for print the model, we can use any prime, it won't be used.
        let execution_plan: ExecutionPlan<MPCProtocol, U64SafePrime> =
            execution_vm_config.plan_strategy.build_plan_without_preprocessing_elements(self.program.body.clone())?;
        protocols_txt.push_str("\n\n");
        protocols_txt.push_str(&execution_plan.text_repr(&self.program.body));
        fs::write(file_path, protocols_txt)?;
        Ok(())
    }
}
