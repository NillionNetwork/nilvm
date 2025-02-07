use anyhow::Error;
use auto_tests::{
    compiler::compile_with_timeout,
    generator::{generate_test_cases, GeneratorOptions},
    runner::TestCase,
};
use clap::Parser;
use colored::Colorize;
use csv::Writer;
use junit_report::{
    Report, ReportBuilder as JUnitReportBuilder, TestCase as JUnitTestCase, TestResult,
    TestSuiteBuilder as JUnitTestSuiteBuilder,
};
use log::debug;
use rayon::prelude::*;
use std::{borrow::Cow, collections::HashMap, env::temp_dir, fmt::Display, fs::File, path::PathBuf};

/// Tool to generate automated tests for NADA language
#[derive(Parser, Debug)]
#[clap(about, version)]
struct Args {
    /// Generate test programs and compile them but do not execute them
    #[clap(short, long)]
    compile_only: bool,
    /// Only generate test programs with one operation in them
    #[clap(short, long)]
    single_ops_only: bool,
    /// Only generate test programs with two operations in them
    #[clap(short, long)]
    double_ops_only: bool,
    /// Output directory
    #[clap(short, long, default_value_t = String::default())]
    output_folder: String,
}

#[derive(Debug, Default)]
struct TestSummary {
    total: u16,
    success: u16,
    failures: u16,
    failed_tests: Vec<JUnitTestCase>,
    failed_compilations: u16,
    ignored_tests: u16,
}

impl TestSummary {
    pub(crate) fn add_test_case_result(&mut self, result: &JUnitTestCase) {
        self.total += 1;
        if result.is_success() {
            self.success += 1
        } else {
            self.failures += 1;
            self.failed_tests.push(result.clone());
        }
    }

    pub(crate) fn add_failed_compilation(&mut self) {
        self.failed_compilations += 1
    }

    pub(crate) fn print_failed_tests(&self) {
        if self.failures > 0 {
            println!("{}", "=== FAILED TESTS ===".bold());
            for failed_test_case in self.failed_tests.iter() {
                println!("{}", failed_test_case.name);
            }
        }
    }
}

impl Display for TestSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TOTAL: {}, OK: {}, FAILURES: {}, IGNORED: {}",
            self.total, self.success, self.failures, self.ignored_tests
        )
    }
}

fn print_failed_compilations(failed_compile_programs: Vec<Cow<'_, str>>) {
    if !failed_compile_programs.is_empty() {
        println!("=== FAILED COMPILATIONS ===");
        println!("{}", failed_compile_programs.join("\n"));
    }
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let args = Args::parse();
    let operations = operations::build();

    debug!("Creating test programs...");
    let output_folder = if args.output_folder.is_empty() {
        temp_dir().join("nada-lang-auto-tests")
    } else {
        PathBuf::from(args.output_folder)
    };

    let (test_cases, ignored_tests) = generate_test_cases(
        operations,
        &output_folder,
        &GeneratorOptions::from_args(args.single_ops_only, args.double_ops_only),
    )?;
    debug!("Compiling test programs...");
    let mut summary = TestSummary { ignored_tests, ..Default::default() };
    let mut failed_compile_programs = vec![];
    let mut compiled_test_cases = vec![];
    for (compilation_result, test_case) in test_cases
        .par_iter()
        .map(|test_case| (compile_with_timeout(test_case, &output_folder), test_case))
        .collect::<Vec<(Result<_, _>, &TestCase)>>()
    {
        match compilation_result {
            Ok(compiled_test_case) => compiled_test_cases.push(compiled_test_case),
            Err(e) => {
                println!("Error compiling test {}: {}", test_case.name, e);
                summary.add_failed_compilation();
                failed_compile_programs.push(test_case.program_path.to_string_lossy());
            }
        }
    }

    if args.compile_only {
        println!("Finished compiling tests, {} failed to compile", summary.failed_compilations);
        print_failed_compilations(failed_compile_programs);
        return Ok(());
    }
    println!("Running {} tests ...", compiled_test_cases.len());

    let junit_test_cases: Vec<JUnitTestCase> = compiled_test_cases
        .par_iter_mut()
        .map(|tc| tc.run())
        .collect::<Vec<JUnitTestCase>>()
        .into_iter()
        .inspect(|result| {
            summary.add_test_case_result(result);
        })
        .collect::<Vec<JUnitTestCase>>();

    println!("{}", format!("Finished running tests: {}", summary).bold());
    summary.print_failed_tests();
    print_failed_compilations(failed_compile_programs);

    let test_suite = JUnitTestSuiteBuilder::new("lang-auto-tests").add_testcases(junit_test_cases.clone()).build();

    let report = JUnitReportBuilder::new().add_testsuite(test_suite).build();
    write_failures_summary("../target/nada-auto-test-summary.csv", &report);
    let report_file = File::create("../target/nada-auto-test.xml").unwrap();
    report.write_xml(report_file).unwrap();
    Ok(())
}

pub(crate) fn write_failures_summary(path: &'static str, runtime_report: &Report) {
    let mut failures_report: HashMap<String, Vec<String>> = HashMap::new();
    let test_cases = runtime_report.testsuites().iter().flat_map(|suite| &suite.testcases);
    for test in test_cases {
        if let TestResult::Failure { message, .. } = &test.result {
            if let Some((error_type, _)) = message.split_once('\n') {
                failures_report.entry(error_type.to_string()).or_default().push(test.name.to_string());
            }
        }
    }
    let mut csv_writer = Writer::from_writer(File::create(path).unwrap());
    for (failure, programs) in failures_report {
        let programs_count = format!("{}", programs.len());
        let programs_list = format!("{:?}", programs);
        csv_writer.write_record(&[failure, programs_count, programs_list]).unwrap();
    }
    csv_writer.flush().unwrap();
}
