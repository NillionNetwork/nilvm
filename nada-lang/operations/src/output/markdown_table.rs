use std::path::Path;

use anyhow::anyhow;

use crate::types::{BuiltOperations, DataType};

struct MarkdownTable {
    result: String,
    column_count: usize,
}

impl MarkdownTable {
    fn new(header: &[&str]) -> anyhow::Result<Self> {
        let mut table = Self { result: String::new(), column_count: header.len() };

        table.add_row(header)?;
        table.add_row(&vec!["---"; header.len()])?;

        Ok(table)
    }

    fn add_row(&mut self, row: &[&str]) -> anyhow::Result<()> {
        if row.len() != self.column_count {
            return Err(anyhow!(
                "expected row of {} entries (based on header size), got {}",
                self.column_count,
                row.len()
            ));
        }

        self.result += &format!("|{}|\n", row.join("|"));

        Ok(())
    }

    fn render(&self) -> &String {
        &self.result
    }
}

/// Generates a Markdown table in a file.
pub fn generate_markdown_tables(operations: &BuiltOperations, filepath: &Path) -> anyhow::Result<()> {
    let mut table = MarkdownTable::new(&["Operation", "Left", "Right", "Output"])?;

    for operation in &operations.binary_operations {
        let operation_name = operation.0;
        table.add_row(&[&format!("{} ({})", operation_name, operation.1.metadata.python_shape), "", "", ""])?;

        let combinations = &operation.1.allowed_combinations;
        let without_combinations = &operation.1.forbidden_combinations;

        let mut possible_combinations = Vec::new();
        let mut impossible_combinations = Vec::new();

        for left in DataType::all_types() {
            for right in DataType::all_types() {
                let output = combinations.get(&(left, right));

                if let Some(output) = output {
                    possible_combinations.push((left.to_string(), right.to_string(), format!("✔️ {}", output)));
                } else {
                    let reason = without_combinations
                        .get(&(left, right))
                        .map(|reason| format!("❌ {}", reason))
                        .unwrap_or_else(|| "❌ unspecified".to_string());
                    impossible_combinations.push((left.to_string(), right.to_string(), reason));
                }
            }
        }

        for (left, right, output) in possible_combinations {
            table.add_row(&["", &left, &right, &output])?;
        }

        for (left, right, reason) in impossible_combinations {
            table.add_row(&["", &left, &right, &reason])?;
        }
    }

    std::fs::write(filepath, table.render())?;

    Ok(())
}
