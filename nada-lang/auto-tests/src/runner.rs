//! The test runner
//!
//! Initially only JUnitTestRunner is implemented but we can build other test runners using the framework

use std::{collections::HashMap, fs, path::PathBuf, time::Instant};

use anyhow::{anyhow, Error, Result};
use colored::Colorize;
use junit_report::{Duration, TestCase as JUnitTestCase, TestCaseBuilder as JUnitTestCaseBuilder};
use log::debug;
use once_cell::sync::Lazy;

use bytecode_evaluator::Evaluator;
use math_lib::modular::U128SafePrime;
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{
        simulator::{InputGenerator, ProgramSimulator, SimulationParameters, StaticInputGeneratorBuilder},
        ExecutionMetricsConfig, ExecutionVmConfig,
    },
    JitCompiler, MPCCompiler, Program, ProgramBytecode,
};
use nada_compiler_backend::mir::ProgramMIR;
use nada_value::{clear::Clear, NadaType, NadaValue};
use pynadac::CompileOutput;

use crate::generator::ProgramInput;

type Prime = U128SafePrime;

static DEFAULT_PARAMETERS: Lazy<SimulationParameters> = Lazy::new(|| SimulationParameters {
    polynomial_degree: 1,
    network_size: 5,
    execution_vm_config: ExecutionVmConfig::default(),
});

pub struct TestCase {
    pub program_path: PathBuf,
    pub name: String,
    pub compile_output: Option<CompileOutput>,
    pub inputs: Vec<ProgramInput>,
}

impl Clone for TestCase {
    fn clone(&self) -> Self {
        Self {
            program_path: self.program_path.clone(),
            name: self.name.clone(),
            compile_output: None,
            inputs: self.inputs.clone(),
        }
    }
}

impl TestCase {
    pub fn run(&mut self) -> JUnitTestCase {
        let start = Instant::now();
        let result = (|| {
            let compile_output = self.compile_output.as_ref().unwrap();
            let program_path = compile_output.mir_bin_file.as_ref().unwrap();
            debug!("Running {}", program_path.to_string_lossy());
            let (program, bytecode) = MPCCompiler::compile_with_bytecode(compile_output.mir.clone())?;
            let input_generator = self.create_inputs(&compile_output.mir, &self.inputs)?;
            let simulator_result = self.simulate(program.clone(), input_generator.clone())?;
            let be_result = self.run_bytecode_evaluator(&bytecode, input_generator.into())?;
            if simulator_result != be_result {
                Err(anyhow::anyhow!(
                    "Program simulator result ({:?}) and bytecode evaluator result ({:?}) do not match",
                    simulator_result,
                    be_result
                ))?
            }
            Ok(())
        })();

        let elapsed = start.elapsed();
        let elapsed = Duration::try_from(elapsed).unwrap();

        match result {
            Ok(_) => JUnitTestCaseBuilder::success(&self.name, elapsed).build(),
            Err(e) => {
                println!("Running test {} ... {}", &self.name, "❌".red());
                JUnitTestCaseBuilder::failure(&self.name, elapsed, "", &self.format_error_message(e)).build()
            }
        }
    }

    fn format_inputs(&self) -> String {
        let mut formatted = "".to_string();
        for input in self.inputs.iter() {
            formatted =
                format!("{formatted}- name: {}, value: {}, type: {}\n", input.name, input.value, input.ty.name());
        }
        formatted
    }

    fn format_error_message(&self, e: Error) -> String {
        format!(
            r#"{e}
==== INPUTS ====
{}
==== PROGRAM LISTING ====
{}
        "#,
            self.format_inputs(),
            fs::read_to_string(&self.program_path).unwrap()
        )
    }

    /// Generates a test Input from a [`NadaType`]
    fn generate_test_input(nada_type: &NadaType, value: &str) -> NadaValue<Clear> {
        // TODO Change so that we can accommodate testing over several test values.
        use NadaType::*;
        match nada_type {
            Integer => {
                let value = value.parse::<i64>().unwrap();
                NadaValue::new_integer(value)
            }
            UnsignedInteger => {
                let value = value.parse::<u64>().unwrap();
                NadaValue::new_unsigned_integer(value)
            }
            SecretInteger => NadaValue::new_secret_integer(value.parse::<i64>().unwrap()),
            SecretUnsignedInteger => NadaValue::new_secret_unsigned_integer(value.parse::<u64>().unwrap()),
            _ => panic!("unexpected nada type {:?}", nada_type),
        }
    }

    /// Create program inputs from MIR
    fn create_inputs(&self, mir: &ProgramMIR, inputs: &[ProgramInput]) -> Result<InputGenerator, Error> {
        let mut input_builder = StaticInputGeneratorBuilder::default();
        let generated_inputs =
            inputs.iter().map(|input| (input.name.clone(), input.clone())).collect::<HashMap<String, ProgramInput>>();
        for input in &mir.inputs {
            let name = &input.name;
            let ty = &input.ty;
            let generated_input =
                generated_inputs.get(name).ok_or_else(|| anyhow!("missing generated input for {name}"))?;
            input_builder.insert(name, Self::generate_test_input(ty, &generated_input.value));
        }
        Ok(input_builder.build())
    }

    /// Runs program simulator and returns result as a [`ModularNumber`]
    fn simulate(
        &self,
        program: Program<MPCProtocol>,
        secrets: InputGenerator,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        let simulator = ProgramSimulator::<MPCProtocol, Prime>::new(
            program,
            DEFAULT_PARAMETERS.clone(),
            &secrets,
            ExecutionMetricsConfig::disabled(),
        )?;
        let (result, _) = simulator.run()?;
        Ok(result)
    }

    fn run_bytecode_evaluator(
        &self,
        bytecode: &ProgramBytecode,
        inputs: HashMap<String, NadaValue<Clear>>,
    ) -> Result<HashMap<String, NadaValue<Clear>>, Error> {
        Evaluator::<Prime>::run(bytecode, inputs)
    }
}
