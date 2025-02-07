//! This files contains all the logic to interact with test cases of static files
//! Static file tests are tests that are defined in a yaml file
//!
use crate::{
    json::{json_to_values, values_to_json},
    nada_project_toml::{NadaProjectToml, ProgramConf},
    program::{get_program, Program},
    run::{run_program, RunOptions},
    test::{parse_json_inputs, TestCase, TestCaseDefinition, TestResult},
};
use colored::Colorize;
use eyre::eyre;
use nada_value::{clear::Clear, NadaType, NadaValue};
use serde::{Deserialize, Serialize};
use serde_json::Value as JSONValue;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    sync::Arc,
};

#[derive(Deserialize, Serialize)]
pub struct StaticTestCaseDefinition {
    #[serde(default, skip_serializing)]
    pub test_name: String,
    pub program: String,
    pub inputs: BTreeMap<String, JSONValue>,
    pub expected_outputs: BTreeMap<String, JSONValue>,
}

impl TestCaseDefinition for StaticTestCaseDefinition {
    fn name(&self) -> String {
        self.test_name.clone()
    }

    fn program_name(&self) -> String {
        self.program.clone()
    }

    fn build(self: Box<Self>, conf: &NadaProjectToml) -> eyre::Result<Box<dyn TestCase>> {
        let StaticTestCaseDefinition { inputs, program, expected_outputs, test_name } = *self;
        let program = Program::build(conf, &program)?;
        let inputs = inputs.into_iter().collect();
        let inputs = parse_json_inputs(&inputs, &program)?;

        let expected_outputs = program
            .program
            .contract
            .output_types()
            .into_iter()
            .map(|(name, output_type)| {
                let value = NadaValue::from_untyped_json(
                    &output_type,
                    expected_outputs.get(&name).cloned().ok_or_else(|| eyre!("output '{name}' not found in test"))?,
                )
                .map_err(|e| eyre!("{e:?}"))?;
                Ok((name, value))
            })
            .collect::<eyre::Result<HashMap<_, _>>>()?;

        Ok(Box::new(StaticTestCase { test_name, program, inputs, expected_outputs }))
    }
}

pub struct StaticTestCase {
    pub test_name: String,
    pub program: Arc<Program>,
    pub inputs: HashMap<String, NadaValue<Clear>>,
    pub expected_outputs: HashMap<String, NadaValue<Clear>>,
}

impl StaticTestCase {
    pub fn assert_test_output(&self, output: HashMap<String, NadaValue<Clear>>) -> StaticTestResult {
        let mut pass = true;
        let mut diff = String::new();
        for (name, value) in self.expected_outputs.iter() {
            if let Some(output_value) = output.get(name) {
                if output_value != value {
                    pass = false;
                    // TODO use diff library to only print diffs
                    diff.push_str(&format!("Output '{}' expected {:?} but got {:?}\n", name, value, output_value));
                }
            } else {
                pass = false;
                diff.push_str(&format!("Output '{}' expected but not found\n", name));
            }
        }
        StaticTestResult { test_name: self.test_name.clone(), pass, diff }
    }
}

impl TestCase for StaticTestCase {
    fn program(&self) -> &Program {
        &self.program
    }

    fn test(&self, debug: bool) -> eyre::Result<Box<dyn TestResult>> {
        let (outputs, _) = self.run(RunOptions {
            debug,
            bytecode_json: false,
            bytecode_text: false,
            protocols_text: false,
            message_size_compute: false,
            execution_plan_metrics: false,
        })?;
        let test_result = self.assert_test_output(outputs);
        Ok(Box::new(test_result))
    }

    fn inputs(&self) -> eyre::Result<HashMap<String, NadaValue<Clear>>> {
        Ok(self.inputs.clone())
    }
}

pub struct StaticTestResult {
    test_name: String,
    pass: bool,
    diff: String,
}

impl Display for StaticTestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.test_name)?;
        if self.pass {
            writeln!(f, "{}", "PASS".green().bold())
        } else {
            writeln!(f, "{}", "FAIL".red().bold())?;
            writeln!(f, "{}", self.diff)
        }
    }
}

impl TestResult for StaticTestResult {
    fn passed(&self) -> bool {
        self.pass
    }
}

pub fn generate_test_file(
    program_conf: &ProgramConf,
    json_inputs: Option<String>,
    json_outputs: Option<String>,
) -> eyre::Result<StaticTestCaseDefinition> {
    let (program, bytecode) = get_program(program_conf)?;
    let inputs_types = program.contract.input_types();

    let mut json_inputs = json_to_values(json_inputs, inputs_types)?;

    let mut inputs = HashMap::new();
    for input in program.contract.inputs.iter() {
        let value = json_inputs.remove(&input.name).map(Ok).unwrap_or_else(|| nada_type_to_nada_value(&input.ty))?;
        inputs.insert(input.name.clone(), value);
    }

    let output_types = program.contract.output_types();

    let json_outputs = json_to_values(json_outputs, output_types)?;

    let run_inputs = inputs.iter().map(|(name, value)| (name.clone(), value.clone())).collect();
    let (mut run_output_values, _) = run_program(
        &Program { conf: program_conf.clone(), bytecode, program },
        run_inputs,
        RunOptions {
            debug: false,
            bytecode_json: false,
            bytecode_text: false,
            protocols_text: false,
            message_size_compute: false,
            execution_plan_metrics: false,
        },
    )?;
    for (name, value) in json_outputs {
        run_output_values.insert(name, value);
    }
    let inputs = values_to_json(inputs)?;
    let ordered_outputs = values_to_json(run_output_values)?;
    Ok(StaticTestCaseDefinition {
        program: program_conf.name.clone(),
        inputs,
        expected_outputs: ordered_outputs,
        test_name: "".to_string(),
    })
}

// TODO We should try to have only values generator and reuse it in all places
fn nada_type_to_nada_value(ty: &NadaType) -> eyre::Result<NadaValue<Clear>> {
    match ty {
        NadaType::Integer => Ok(NadaValue::new_integer(3)),
        NadaType::UnsignedInteger => Ok(NadaValue::new_unsigned_integer(3u32)),
        NadaType::Boolean => Ok(NadaValue::new_boolean(true)),
        NadaType::SecretInteger => Ok(NadaValue::new_secret_integer(3)),
        NadaType::SecretUnsignedInteger => Ok(NadaValue::new_secret_unsigned_integer(3u32)),
        NadaType::SecretBoolean => Ok(NadaValue::new_secret_boolean(true)),
        ty @ NadaType::SecretBlob
        | ty @ NadaType::EcdsaDigestMessage
        | ty @ NadaType::ShamirShareInteger
        | ty @ NadaType::ShamirShareUnsignedInteger
        | ty @ NadaType::ShamirShareBoolean
        | ty @ NadaType::EcdsaPrivateKey
        | ty @ NadaType::EcdsaSignature => Err(eyre!("Unsupported type: {:?}", ty)),
        NadaType::Array { inner_type, size } => {
            let value = nada_type_to_nada_value(&inner_type.clone())?;
            Ok(NadaValue::new_array(*inner_type.clone(), vec![value; *size])?)
        }
        NadaType::Tuple { left_type, right_type } => {
            Ok(NadaValue::new_tuple(nada_type_to_nada_value(left_type)?, nada_type_to_nada_value(right_type)?)?)
        }
        NadaType::NTuple { types } => {
            Ok(NadaValue::new_n_tuple(types.iter().map(nada_type_to_nada_value).collect::<eyre::Result<_>>()?)?)
        }
        NadaType::Object { types } => Ok(NadaValue::new_object(
            types
                .iter()
                .map(|(key, value)| nada_type_to_nada_value(value).map(|value| (key.clone(), value)))
                .collect::<eyre::Result<_>>()?,
        )?),
    }
}
