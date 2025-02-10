//! Reporting for executed tests.

use crate::flow::{FlowKind, FlowSummary};
use csv::Writer;
use log::info;
use serde::Serialize;
use std::{path::Path, time::Duration};

/// A report for an executed flow.
pub struct FlowReport {
    /// The flow's summary.
    pub summary: FlowSummary,

    /// The amount of time elapsed since the entire load test execution began.
    pub elapsed_since_start: Duration,

    /// The number of workers that were executing flows at the time this flow finished.
    pub total_workers: u32,
}

/// Metadata about a report.
#[derive(Serialize)]
pub struct FlowMetadata {
    /// The kind of flow this test executed.
    pub kind: FlowKind,

    /// The number of secrets stored/retrieved per flow.
    pub secret_count: u32,

    /// The size of the secrets being stored/retrieved in bytes.
    pub secrets_size: u32,
}

#[derive(Serialize)]
struct Report {
    metadata: FlowMetadata,
    content: String,
}

/// A report generator.
pub struct ReportGenerator;

impl ReportGenerator {
    /// Write the report into the given file path.
    pub fn write_report(output_path: &Path, metadata: FlowMetadata, reports: Vec<FlowReport>) -> anyhow::Result<()> {
        let content = Self::serialize_reports(&reports)?;
        let report = Report { metadata, content };
        serde_files_utils::json::write_json(output_path, &report)?;

        info!("Load test report has been saved successfully to {output_path:?}");
        Ok(())
    }

    fn serialize_reports(reports: &[FlowReport]) -> anyhow::Result<String> {
        let mut writer = Writer::from_writer(vec![]);
        writer.write_record([
            "Id",
            "Elapsed",
            "Total workers",
            "State",
            "Quote Duration",
            "Payment Duration",
            "Operation Duration",
            "Duration",
        ])?;
        for (index, report) in reports.iter().enumerate() {
            writer.write_record(&[
                index.to_string(),
                report.elapsed_since_start.as_millis().to_string(),
                report.total_workers.to_string(),
                format!("{:?}", report.summary.status),
                report.summary.quote_duration.as_millis().to_string(),
                report.summary.payment_duration.as_millis().to_string(),
                report.summary.operation_duration.as_millis().to_string(),
                report.summary.elapsed.as_millis().to_string(),
            ])?;
        }
        let buffer = writer.into_inner()?;
        Ok(String::from_utf8(buffer)?)
    }
}
