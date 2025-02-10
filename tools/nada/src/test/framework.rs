//! Test Framework module
//! This files contains all the logic to interact with test cases of test frameworks
//! The test frameworks are external programs that can be used to run tests they are defined in the nada project toml file
//! The test frameworks are expected to have the following commands:
//! - list: list all the tests
//! - inputs: get the inputs for a test
//! - test: run a test
//!   The test frameworks are expected to output json
//!   The test frameworks are expected to have the following environment variables:
//! - NADA_PROJECT_ROOT: the root of the nada project
//! - NADA_TEST_COMMAND: the command to run the test framework
//! - NADA_TEST_NAME: the name of the test to run
//! - NADA_TEST_DEBUG: whether to run the test in debug mode
//!

use crate::{
    nada_project_toml::{NadaProjectToml, TestFramework},
    paths::get_nada_project_root,
    program::Program,
    test::{parse_json_inputs, TestCase, TestCaseDefinition, TestResult},
};
use colored::Colorize;
use eyre::{eyre, Result, WrapErr};
use nada_value::{clear::Clear, NadaValue};
use serde::Deserialize;
use serde_json::Value as JSONValue;
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fmt::{Display, Formatter},
    process::{Command, Output},
    sync::Arc,
};

impl TestFramework {
    pub fn list(&self, test_framework_name: &str) -> Result<Vec<Box<dyn TestCaseDefinition>>> {
        let out = self.run_cmd::<[(&str, &str); 0], _, _>("list", [])?;
        Self::check_cmd_output(&out, "list")?;

        #[derive(Deserialize)]
        struct TestCaseListElement {
            name: String,
            program: String,
        }

        let tests: Vec<TestCaseListElement> = serde_json::from_slice(&out.stdout).with_context(|| {
            format!(
                "Deserializing stdout from test framework list command: {}",
                String::from_utf8(out.stdout).unwrap_or("Invalid utf-8".to_string())
            )
        })?;

        Ok(tests
            .into_iter()
            .map(|test| {
                Box::new(FrameworkTestCaseDefinition {
                    test_framework_name: test_framework_name.to_string(),
                    test_framework: self.clone(),
                    test_name: test.name,
                    program_name: test.program,
                }) as Box<dyn TestCaseDefinition>
            })
            .collect())
    }

    pub fn inputs(&self, test_name: &str) -> Result<HashMap<String, JSONValue>> {
        let out = self.run_cmd("inputs", [("NADA_TEST_NAME", test_name)])?;
        Self::check_cmd_output(&out, "inputs")?;
        let inputs: HashMap<String, JSONValue> = serde_json::from_slice(&out.stdout).with_context(|| {
            format!(
                "Deserializing stdout from test framework inputs command: {}",
                String::from_utf8(out.stdout).unwrap_or("Invalid utf-8".to_string())
            )
        })?;
        Ok(inputs)
    }

    fn check_cmd_output(out: &Output, command: &str) -> Result<()> {
        if !out.status.success() {
            return Err(eyre!(
                "Error running test framework command {command}:\n---------stdout---------\n{}\n------------------------\n---------stderr---------\n{}\n------------------------",
                String::from_utf8(out.stdout.clone()).unwrap_or("Invalid utf-8".to_string()),
                String::from_utf8(out.stderr.clone()).unwrap_or("Invalid utf-8".to_string())
            ));
        }
        Ok(())
    }

    pub fn test(&self, test_name: &str, debug: bool) -> Result<(bool, String, String)> {
        let debug = if debug { "true" } else { "false" };
        let out = self.run_cmd("test", [("NADA_TEST_NAME", test_name), ("NADA_TEST_DEBUG", debug)])?;
        let stdout = String::from_utf8(out.stdout)?;
        let stderr = String::from_utf8(out.stderr)?;
        let passed = out.status.success();
        Ok((passed, stdout, stderr))
    }

    fn run_cmd<I, K, V>(&self, command: &str, extra_args: I) -> Result<Output>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        Command::new(&shell)
            .env("NADA_PROJECT_ROOT", &get_nada_project_root()?)
            .env("NADA_TEST_COMMAND", command)
            .envs(extra_args)
            .arg("-c")
            .arg(&self.command)
            .output()
            .context("Error running test framework")
    }
}

pub struct FrameworkTestCaseDefinition {
    test_framework_name: String,
    test_framework: TestFramework,
    test_name: String,
    program_name: String,
}

impl TestCaseDefinition for FrameworkTestCaseDefinition {
    fn name(&self) -> String {
        format!("{}::{}", self.test_framework_name, self.test_name)
    }

    fn program_name(&self) -> String {
        self.program_name.clone()
    }

    fn build(self: Box<Self>, conf: &NadaProjectToml) -> Result<Box<dyn TestCase>> {
        let full_test_name = self.name();
        let FrameworkTestCaseDefinition { test_framework, test_name, program_name, .. } = *self;
        let program = Program::build(conf, &program_name)?;
        Ok(Box::new(FrameworkTestCase { test_framework, test_name, program, full_test_name }))
    }
}

pub struct FrameworkTestCase {
    full_test_name: String,
    test_framework: TestFramework,
    test_name: String,
    program: Arc<Program>,
}

impl TestCase for FrameworkTestCase {
    fn program(&self) -> &Program {
        &self.program
    }

    fn test(&self, debug: bool) -> Result<Box<dyn TestResult>> {
        let (passed, stdout, stderr) = self.test_framework.test(&self.test_name, debug)?;
        Ok(Box::new(TestFrameworkTestResult { passed, full_test_name: self.full_test_name.clone(), stdout, stderr }))
    }

    fn inputs(&self) -> Result<HashMap<String, NadaValue<Clear>>> {
        let inputs_json = self.test_framework.inputs(&self.test_name)?;
        parse_json_inputs(&inputs_json, &self.program)
    }
}

pub struct TestFrameworkTestResult {
    full_test_name: String,
    passed: bool,
    stdout: String,
    stderr: String,
}

impl Display for TestFrameworkTestResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.full_test_name)?;
        if self.passed {
            writeln!(f, "{}", "PASS".green().bold())
        } else {
            writeln!(f, "{}", "FAIL".red().bold())?;
            if !self.stdout.is_empty() {
                writeln!(f, "---------stdout---------\n{}\n------------------------", self.stdout)?;
            }
            if !self.stderr.is_empty() {
                writeln!(f, "---------stderr---------\n{}\n------------------------", self.stderr)?;
            }
            Ok(())
        }
    }
}

impl TestResult for TestFrameworkTestResult {
    fn passed(&self) -> bool {
        self.passed
    }
}
