use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use crate::{args::BenchmarkArgs, nada_project_toml::NadaProjectToml, run::RunOptions, test, test::TestCase, Runner};
use color_eyre::owo_colors::OwoColorize;
use colored::Colorize;
use eyre::{eyre, Result};
use humansize::FormatSizeOptions;
use indicatif::ProgressBar;
use mpc_vm::vm::simulator::ExecutionMetrics;
use tabled::{
    builder::Builder,
    settings::{Panel, Span, Style},
};

/// Stores extrema values, and allows to mark an input value as best or worst using color.
#[derive(Default)]
struct BenchmarkValue<T> {
    /// Minimum value, if any.
    min: Option<T>,

    /// Maximum value, if any.
    max: Option<T>,

    /// Last value, if any.
    last_value: Option<T>,

    /// Are there different values, or are there all the same?
    /// If the values are all the same we don't display the best or worst value.
    different_values: bool,
}

impl<T> BenchmarkValue<T>
where
    T: Clone + Copy + Default + PartialOrd,
{
    /// Updates the min, max and last values.
    pub fn update(&mut self, value: T) {
        if let Some(min) = &mut self.min {
            if value < *min {
                *min = value;
            }
        } else {
            self.min = Some(value);
        }

        if let Some(max) = &mut self.max {
            if value > *max {
                *max = value;
            }
        } else {
            self.max = Some(value);
        }

        if let Some(last_value) = self.last_value {
            if last_value != value {
                self.different_values = true;
            }
        } else {
            self.last_value = Some(value);
        }
    }

    pub fn is_best(&self, value: T) -> bool {
        if !self.different_values {
            return false;
        }

        if let Some(min) = self.min { min == value } else { false }
    }

    pub fn is_worst(&self, value: T) -> bool {
        if !self.different_values {
            return false;
        }

        if let Some(max) = self.max { max == value } else { false }
    }

    /// Renders a value using color and other attributes depending on it's value relative to the extrema.
    pub fn to_string(&self, value: T) -> String
    where
        T: ToString,
    {
        self.to_string_internal(value, value.to_string())
    }

    /// Renders a value using color and other attributes depending on it's value relative to the extrema.
    /// This variant uses a formatter to transform the value into a string.
    pub fn to_string_with_formatter<D, F>(&self, value: T, value_formatter: F) -> String
    where
        D: ToString,
        F: FnOnce(T) -> D,
    {
        self.to_string_internal(value, value_formatter(value).to_string())
    }

    fn to_string_internal(&self, value: T, value_string: String) -> String {
        let value_string = if value == T::default() { "none".italic().to_string() } else { value_string.to_string() };

        if self.is_best(value) {
            format!("{value_string} ⭐").green().bold().to_string()
        } else if self.is_worst(value) {
            value_string.red().to_string()
        } else {
            value_string
        }
    }
}

type BenchmarksResults = BTreeMap<String, ExecutionMetrics>;

impl Runner {
    pub fn benchmark(args: &BenchmarkArgs) -> Result<()> {
        let conf = NadaProjectToml::find_self()?;

        let test_files = if let Some(tests) = &args.tests {
            test::get_all_test_case_definition(&conf)?
                .into_iter()
                .filter(|(name, _)| tests.contains(name))
                .collect::<BTreeMap<_, _>>()
        } else {
            // Get all test cases, keep the lexicographic order
            test::get_all_test_case_definition(&conf)?
        };

        if test_files.is_empty() {
            return Err(eyre!("At least one test is required"));
        }
        if args.run_count < 1 {
            return Err(eyre!("At least one run is required per test"));
        }

        // Prepare test data: collect the names, configuration and inputs
        let tests = test_files
            .into_iter()
            .map(|(test_name, test_file)| -> Result<_> {
                let test_case = test_file.build(&conf)?;
                Ok((test_name, test_case))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let results = run_tests(&tests, args)?;

        draw_summary_table(&results, args);

        if results.values().any(|metrics| !metrics.summary.preprocessing_elements.is_empty()) {
            draw_preprocessing_elements_table(&results);
        }

        if results.values().any(|metrics| !metrics.summary.local_protocols.is_empty()) {
            draw_local_protocols_table(&results);
        }

        if results.values().any(|metrics| !metrics.summary.online_protocols.is_empty()) {
            draw_online_protocols_table(&results, args);
        }

        Ok(())
    }
}

fn run_tests(
    tests: &BTreeMap<String, Box<dyn TestCase>>,
    args: &BenchmarkArgs,
) -> Result<BTreeMap<String, ExecutionMetrics>> {
    println!("Running...");
    let bar = ProgressBar::new((tests.len() * args.run_count) as u64);

    let results = tests
        .iter()
        .map(|(name, test_case)| {
            let mut run_metrics = Vec::with_capacity(args.run_count);

            for _ in 0..args.run_count {
                let (_, metrics) = test_case.run(RunOptions {
                    debug: false,
                    mir_text: false,
                    bytecode_json: false,
                    bytecode_text: false,
                    protocols_text: false,
                    message_size_compute: args.message_size_calculation,
                    execution_plan_metrics: false,
                })?;
                run_metrics.push(metrics.expect("expected metrics result for test run"));

                bar.inc(1);
            }

            let metrics = ExecutionMetrics::merge(run_metrics).expect("failed merging metrics results");

            Ok((name.clone(), metrics))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    bar.finish();

    println!();
    println!();

    Ok(results)
}

fn draw_summary_table(results: &BenchmarksResults, args: &BenchmarkArgs) {
    // TODO Not sure if we should print information about execution steps here. It seems like
    //  this metric is more useful over the protocols model.
    let mut headers = vec!["Test", "Total Duration", "Compute Duration", "Rounds"];
    if args.message_size_calculation {
        headers.push("Total message size");
    }
    headers.push("Preprocessing elements");
    headers.push("Local protocols");
    headers.push("Online protocols");

    let mut builder = Builder::default();
    builder.push_record(headers.into_iter().map(|header| header.bold().to_string()));

    let mut execution_duration_value = BenchmarkValue::default();
    let mut total_compute_duration = BenchmarkValue::default();
    let mut total_rounds = BenchmarkValue::default();
    let mut total_message_size = BenchmarkValue::default();
    let mut preprocessing_elements_value = BenchmarkValue::default();
    let mut local_protocols_value = BenchmarkValue::default();
    let mut online_protocols_value = BenchmarkValue::default();

    // Update extrema values
    for metrics in results.values() {
        execution_duration_value.update(metrics.summary.execution_duration);
        total_compute_duration.update(metrics.summary.compute_duration.total);
        total_rounds.update(metrics.summary.total_rounds);
        total_message_size.update(metrics.summary.total_message_size);
        let preprocessing_elements_count: usize = metrics.summary.preprocessing_elements.values().copied().sum();
        preprocessing_elements_value.update(preprocessing_elements_count);
        local_protocols_value.update(metrics.summary.local_protocols.len());
        online_protocols_value.update(metrics.summary.online_protocols.len());
    }

    // Generate the table's rows
    for (name, metrics) in results.iter() {
        let mut row = vec![
            name.bold().to_string(),
            execution_duration_value
                .to_string_with_formatter(metrics.summary.execution_duration, humantime::format_duration),
            total_compute_duration
                .to_string_with_formatter(metrics.summary.compute_duration.total, humantime::format_duration),
            total_rounds.to_string(metrics.summary.total_rounds),
        ];
        if args.message_size_calculation {
            row.push(total_message_size.to_string_with_formatter(metrics.summary.total_message_size, |value| {
                humansize::format_size(value.unwrap_or_default(), FormatSizeOptions::default())
            }));
        }
        row.push(
            preprocessing_elements_value.to_string(metrics.summary.preprocessing_elements.values().copied().sum()),
        );
        row.push(local_protocols_value.to_string(metrics.summary.local_protocols.len()));
        row.push(online_protocols_value.to_string(metrics.summary.online_protocols.len()));
        builder.push_record(row);
    }

    let mut table = builder.build();
    table
        .with(Panel::header("Benchmark summary".bold().to_string()))
        .with(Panel::footer(format!("{} run(s) per protocol", args.run_count)))
        .with(Style::modern_rounded());

    println!("{table}");
    println!();
}

fn draw_preprocessing_elements_table(results: &BenchmarksResults) {
    let mut preprocessing_elements = BTreeMap::new();

    // Collect the names of all preprocessing elements
    for metrics in results.values() {
        for preprocessing_element in metrics.summary.preprocessing_elements.keys() {
            preprocessing_elements.insert(preprocessing_element.clone(), BenchmarkValue::default());
        }
    }

    // Update the extrema values, add 0 if a particular preprocessing elements has not been used
    for metrics in results.values() {
        for (name, total_count) in &mut preprocessing_elements {
            let count = metrics.summary.preprocessing_elements.get(name).copied().unwrap_or(0);
            total_count.update(count);
        }
    }

    let mut headers = vec!["Test".to_string()];

    for preprocessing_element in preprocessing_elements.keys() {
        headers.push(format!("{preprocessing_element:?}"));
    }

    let mut builder = Builder::default();
    builder.push_record(headers.into_iter().map(|header| header.bold().to_string()));

    for (name, metrics) in results.iter() {
        let mut row = vec![name.bold().to_string()];

        for (element, min_max) in &preprocessing_elements {
            let count = metrics.summary.preprocessing_elements.get(element).copied().unwrap_or(0);
            row.push(min_max.to_string(count));
        }

        builder.push_record(row);
    }

    let mut table = builder.build();
    table.with(Panel::header("Preprocessing elements".bold().to_string())).with(Style::modern_rounded());

    println!("{table}");
    println!();
}

fn draw_local_protocols_table(results: &BenchmarksResults) {
    let mut protocol_names = BTreeSet::new();

    // Collect the names of all the protocols
    for metrics in results.values() {
        for protocol in metrics.summary.local_protocols.keys() {
            protocol_names.insert(protocol);
        }
    }

    let mut builder = Builder::default();

    for protocol_name in &protocol_names {
        let mut total_duration_value = BenchmarkValue::default();
        let mut calls_value = BenchmarkValue::default();

        // Update the extrema values, add 0 if a particular protocol has not been used
        for metrics in results.values() {
            let local_protocols = &metrics.summary.local_protocols;
            let (total_duration, calls) =
                local_protocols.get(*protocol_name).map(|p| (p.duration.total, p.calls)).unwrap_or((Duration::ZERO, 0));

            total_duration_value.update(total_duration);
            calls_value.update(calls);
        }

        // Use columns here so we can join all protocols in the same table
        let mut names_column = vec![protocol_name.bold().to_string(), "Test".bold().to_string()];
        let mut total_duration_column = vec![String::new(), "Duration".bold().to_string()];
        let mut calls_column = vec![String::new(), "Calls".bold().to_string()];

        for (name, metrics) in results.iter() {
            names_column.push(name.bold().to_string());

            let local_protocols = &metrics.summary.local_protocols;
            let (total_duration, calls) =
                local_protocols.get(*protocol_name).map(|p| (p.duration.total, p.calls)).unwrap_or((Duration::ZERO, 0));

            total_duration_column
                .push(total_duration_value.to_string_with_formatter(total_duration, humantime::format_duration));
            calls_column.push(calls_value.to_string(calls));
        }

        builder.push_column(names_column);
        builder.push_column(total_duration_column);
        builder.push_column(calls_column);
    }

    let mut table = builder.build();

    // Create a table span for the protocol names
    for i in 0..protocol_names.len() {
        table.modify((0, i * 3), Span::column(3));
    }

    table.with(Panel::header("Local protocols".bold().to_string())).with(Style::modern_rounded());

    println!("{table}");
    println!();
}

fn draw_online_protocols_table(results: &BenchmarksResults, args: &BenchmarkArgs) {
    let mut protocol_names = BTreeSet::new();

    // Collect the names of all the protocols
    for metrics in results.values() {
        for protocol_name in metrics.summary.online_protocols.keys() {
            protocol_names.insert(protocol_name);
        }
    }

    let mut builder = Builder::default();

    for protocol_name in &protocol_names {
        let mut total_duration_value = BenchmarkValue::default();
        let mut calls_value = BenchmarkValue::default();
        let mut total_message_size_size_value = BenchmarkValue::default();

        // Update the extrema values, add 0 if a particular protocol has not been used
        for metrics in results.values() {
            let online_protocols = &metrics.summary.online_protocols;
            let (total_duration, calls, message_size) = online_protocols
                .get(*protocol_name)
                .map(|p| (p.duration.total, p.calls, p.total_message_size))
                .unwrap_or((Duration::ZERO, 0, 0));

            total_duration_value.update(total_duration);
            calls_value.update(calls);
            total_message_size_size_value.update(message_size);
        }

        // Use columns here so we can join all protocols in the same table
        let mut names_column = vec![protocol_name.bold().to_string(), "Test".bold().to_string()];
        let mut total_duration_column = vec![String::new(), "Duration".bold().to_string()];
        let mut calls_column = vec![String::new(), "Calls".bold().to_string()];
        let mut total_message_size_size_column = vec![String::new(), "Total message size".bold().to_string()];

        for (name, metrics) in results.iter() {
            names_column.push(name.bold().to_string());

            let online_protocols = &metrics.summary.online_protocols;
            let (total_duration, calls, total_message_size) = online_protocols
                .get(*protocol_name)
                .map(|p| (p.duration.total, p.calls, p.total_message_size))
                .unwrap_or((Duration::ZERO, 0, 0));

            total_duration_column
                .push(total_duration_value.to_string_with_formatter(total_duration, humantime::format_duration));
            calls_column.push(calls_value.to_string(calls));

            if args.message_size_calculation {
                total_message_size_size_column.push(
                    total_message_size_size_value.to_string_with_formatter(total_message_size, |value| {
                        humansize::format_size(value, FormatSizeOptions::default())
                    }),
                );
            }
        }

        builder.push_column(names_column);
        builder.push_column(total_duration_column);
        builder.push_column(calls_column);
        if args.message_size_calculation {
            builder.push_column(total_message_size_size_column);
        }
    }

    let mut table = builder.build();

    // Create a table span for the protocol names
    let column_count = if args.message_size_calculation { 5 } else { 4 };
    for i in 0..protocol_names.len() {
        table.modify((0, i * column_count), Span::column(column_count));
    }

    table.with(Panel::header("Online protocols".bold().to_string())).with(Style::modern_rounded());

    println!("{table}");
    println!();
}
