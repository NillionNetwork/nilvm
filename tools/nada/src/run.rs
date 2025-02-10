use std::collections::HashMap;

use eyre::Result;

use crate::{error::IntoEyre, program::Program};
use bytecode_evaluator::EvaluatorRunner;
use math_lib::modular::EncodedModulo;
use mpc_vm::vm::{
    simulator::{ExecutionMetrics, InputGenerator, SimulationParameters, SimulatorRunner},
    ExecutionMetricsConfig, ExecutionVmConfig,
};
use nada_value::{clear::Clear, NadaValue};

pub struct RunOptions {
    pub debug: bool,
    pub mir_text: bool,
    pub bytecode_json: bool,
    pub bytecode_text: bool,
    pub protocols_text: bool,
    pub message_size_compute: bool,
    pub execution_plan_metrics: bool,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn run_program(
    program: &Program,
    inputs: HashMap<String, NadaValue<Clear>>,
    options: RunOptions,
) -> Result<(HashMap<String, NadaValue<Clear>>, Option<ExecutionMetrics>)> {
    let parameters = SimulationParameters {
        network_size: 5,
        polynomial_degree: 1,
        execution_vm_config: ExecutionVmConfig::default(),
    };

    if options.mir_text {
        program.mir_to_text()?;
    }

    if options.bytecode_json {
        program.bytecode_to_json()?;
    }

    if options.bytecode_text {
        program.bytecode_to_text()?;
    }
    if options.protocols_text {
        program.protocols_to_text(&parameters.execution_vm_config)?;
    }

    let encoded_safe_prime = EncodedModulo::try_safe_prime_from_bits(program.conf.prime_size)?;

    let result = if options.debug {
        let runner = Box::<dyn EvaluatorRunner>::try_from(&encoded_safe_prime)?;
        let result = runner.run(&program.bytecode, inputs).into_eyre()?;
        (result, None)
    } else {
        let inputs = InputGenerator::Static(inputs);
        let runner = Box::<dyn SimulatorRunner>::try_from(&encoded_safe_prime)?;
        let (result, metrics) = runner
            .run(
                program.program.clone(),
                parameters,
                &inputs,
                ExecutionMetricsConfig::enabled(options.message_size_compute, options.execution_plan_metrics),
            )
            .into_eyre()?;
        (result, Some(metrics))
    };

    Ok(result)
}
