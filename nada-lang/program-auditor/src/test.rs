//! Tests for the program auditor

use crate::{MaxPreprocessingPolicy, ProgramAuditorError, ProgramAuditorRequest};
use anyhow::Error;
use mpc_vm::requirements::MPCProgramRequirements;
use nada_compiler_backend::mir::NamedElement;
use rstest::rstest;
use test_programs::PROGRAMS;

use crate::{ProgramAuditor, ProgramAuditorConfig};

fn good_config() -> ProgramAuditorConfig {
    let config = ProgramAuditorConfig {
        max_memory_size: 100,
        max_instructions: 100,
        max_instructions_per_type: vec![("Addition".to_string(), 100u64), ("MultiplicationShares".to_string(), 100u64)]
            .into_iter()
            .collect(),
        max_preprocessing: MPCProgramRequirements::default()
            .with_compare_elements(10)
            .with_division_integer_secret_elements(10)
            .with_equals_integer_secret_elements(10)
            .with_modulo_elements(10)
            .with_public_output_equality_elements(10)
            .with_trunc_elements(10)
            .with_truncpr_elements(10),
        ..Default::default()
    };
    println!("{config:#?}");
    config
}

/// Matches the configuration for functional tests.
fn functional_tests_config() -> ProgramAuditorConfig {
    let config = ProgramAuditorConfig {
        max_memory_size: 3006,
        max_instructions: 1002,
        max_instructions_per_type: vec![("Addition".to_string(), 100u64), ("MultiplicationShares".to_string(), 100u64)]
            .into_iter()
            .collect(),
        max_preprocessing: MPCProgramRequirements::default()
            .with_compare_elements(100)
            .with_division_integer_secret_elements(100)
            .with_equals_integer_secret_elements(100)
            .with_modulo_elements(100)
            .with_public_output_equality_elements(100)
            .with_trunc_elements(100)
            .with_truncpr_elements(100),
        ..Default::default()
    };
    config
}

fn run_test_program_auditor(
    program: &str,
    config: ProgramAuditorConfig,
    success: bool,
    policy_failure: Option<String>,
) -> Result<(), Error> {
    let mir = PROGRAMS.mir(program)?;
    let auditor = ProgramAuditor::new(config);
    let auditor_request = ProgramAuditorRequest::from_mir(&mir)?;
    let audit_result = auditor.audit(&auditor_request);
    if success {
        assert!(audit_result.is_ok());
    } else {
        if let Some(failure) = policy_failure {
            assert!(audit_result.is_err());
            if let Err(ProgramAuditorError::InvalidProgram(actual_violation)) = audit_result {
                println!("Actual policy violation: {:?}, {}", actual_violation.policy, actual_violation.message);
                assert_eq!(failure, actual_violation.policy);
            } else {
                panic!("expecting invalid program error, found {:?}", audit_result);
            }
        } else {
            panic!("Wrong test setup, expecting failure string, got none");
        }
    }
    Ok(())
}

#[rstest]
#[ignore = "functions are broken in MIR preprocessing"]
#[case::array_product_ok("array_product", good_config(), true, None)]
#[case::invalid_program("invalid_program", functional_tests_config(), false, Some(format!("{}[DivisionIntegerSecret]",MaxPreprocessingPolicy.name())))]
fn test_program_auditor(
    #[case] program: &str,
    #[case] config: ProgramAuditorConfig,
    #[case] success: bool,
    #[case] policy_failure: Option<String>,
) -> Result<(), Error> {
    run_test_program_auditor(program, config, success, policy_failure)
}

#[test]
fn test_default_config_enabled() {
    let config = ProgramAuditorConfig::default();
    assert!(!config.disable);
}
