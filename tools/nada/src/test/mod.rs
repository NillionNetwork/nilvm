use crate::{
    error::IntoEyre,
    nada_project_toml::NadaProjectToml,
    paths,
    program::Program,
    run::{run_program, RunOptions},
};
use eyre::{eyre, Result, WrapErr};
use mpc_vm::vm::simulator::ExecutionMetrics;
use nada_value::{clear::Clear, NadaValue};
use serde_files_utils::yaml::read_yaml;
use serde_json::Value;
use staticfile::StaticTestCaseDefinition;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
    fs,
};

mod framework;
pub mod staticfile;

/// Find a test case definition by name
pub fn find_test_case_definition(test_name: &str) -> Result<Box<dyn TestCaseDefinition>> {
    get_all_test_case_definition(&NadaProjectToml::find_self()?)?
        .remove(test_name)
        .ok_or_else(|| eyre!("Test case not found"))
}

/// Get all the test case definitions from the test files and the test frameworks
pub fn get_all_test_case_definition(config: &NadaProjectToml) -> Result<BTreeMap<String, Box<dyn TestCaseDefinition>>> {
    let mut test_cases: BTreeMap<String, Box<dyn TestCaseDefinition>> = BTreeMap::new();
    let tests_path = paths::get_tests_path()?;
    for file in fs::read_dir(&tests_path)? {
        let file = file?;
        let path = file.path();
        let Some(extension) = path.extension() else { continue };
        if extension != "yaml" && extension != "yml" {
            continue;
        }
        let test_name = path.file_stem().ok_or_else(|| eyre!("Error getting file name"))?.to_string_lossy().to_string();
        let mut test_file = read_yaml::<_, StaticTestCaseDefinition>(&path)
            .into_eyre()
            .with_context(|| format!("Error deserializing test case file {}", path.to_string_lossy()))?;
        test_file.test_name = test_name.clone();
        test_cases.insert(test_name, Box::new(test_file));
    }

    for (name, test_framework) in config.test_framework.iter() {
        let test_framework_tests = test_framework.list(name)?;

        for test in test_framework_tests {
            test_cases.insert(test.name(), test);
        }
    }
    Ok(test_cases)
}

/// A Test Case definition that defines a test case, it is not yet built,
/// so it only contains the name of the test and the name of the program,
/// and can be built into a TestCase
pub trait TestCaseDefinition {
    fn name(&self) -> String;
    fn program_name(&self) -> String;
    fn build(self: Box<Self>, conf: &NadaProjectToml) -> Result<Box<dyn TestCase>>;
}

/// A TestCase that have a program, inputs and can be run or tested
pub trait TestCase {
    fn program(&self) -> &Program;

    #[allow(clippy::type_complexity)]
    fn run(&self, options: RunOptions) -> Result<(HashMap<String, NadaValue<Clear>>, Option<ExecutionMetrics>)> {
        run_program(self.program(), self.inputs()?, options)
    }
    fn test(&self, debug: bool) -> Result<Box<dyn TestResult>>;
    fn inputs(&self) -> Result<HashMap<String, NadaValue<Clear>>>;
}

/// A Test Result that can be displayed and have a pass status
pub trait TestResult: Display {
    fn passed(&self) -> bool;
}

pub fn parse_json_inputs(
    inputs: &HashMap<String, Value>,
    program: &Program,
) -> Result<HashMap<String, NadaValue<Clear>>> {
    program
        .program
        .contract
        .input_types()
        .into_iter()
        .map(|(name, input_type)| {
            let value = NadaValue::from_untyped_json(
                &input_type,
                inputs.get(&name).cloned().ok_or_else(|| eyre!("input '{name}' not found in test"))?,
            )
            .map_err(|e| eyre!("{e:?}"))?;
            Ok((name, value))
        })
        .collect::<Result<HashMap<_, _>>>()
}
