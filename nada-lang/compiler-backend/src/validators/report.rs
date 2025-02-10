//! MIR validation reporting utilities
use anyhow::{anyhow, Result};
use ariadne::{Cache, Color, Fmt, FnCache, Label, Report, ReportKind, Source};
use mir_model::{NamedElement, ProgramMIR, SourceInfo, SourceRef, TypedElement};
use nada_value::NadaType;
use std::fmt::{Display, Formatter};

/// Issue message
pub enum IssueMessage {
    /// Message variant
    Message(String, SourceRef),
    /// Note variant
    Note(String),
}

impl Display for IssueMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use IssueMessage::*;
        match self {
            Message(msg, _) => write!(f, "{msg}"),
            Note(note) => write!(f, "{note}"),
        }
    }
}

/// Validation context.
///
/// Contains the list of issues found during validation.
#[derive(Default)]
pub struct ValidationContext {
    issues: Vec<(Vec<IssueMessage>, SourceRef)>,
}

impl From<ValidationContext> for Vec<String> {
    fn from(value: ValidationContext) -> Self {
        use IssueMessage::*;
        let mut messages = vec![];
        for (issue_messages, _) in value.issues {
            let mut message = String::from("");
            for issue_message in issue_messages {
                match issue_message {
                    Message(_, _) => {
                        let separator = if !message.is_empty() { ": " } else { "" };
                        message = format!("{message}{separator}{issue_message}");
                    }
                    Note(_) => message = format!("{message} ({issue_message})"),
                }
            }
            messages.push(message);
        }
        messages
    }
}

/// Label color
pub const LABEL_COLOR: Color = Color::Red;
/// Note color
pub const NOTE_COLOR: Color = Color::Cyan;

impl ValidationContext {
    pub(crate) fn report_incompatible_type<L: TypedElement, R: TypedElement + SourceInfo>(
        &mut self,
        expected: &L,
        found: &R,
        program: &ProgramMIR,
    ) -> Result<()> {
        self.report_incompatible_type_with_source_ref(
            expected.ty(),
            found.ty(),
            program.source_ref(found.source_ref_index())?,
        );

        Ok(())
    }

    pub(crate) fn report_incompatible_type_with_source_ref(
        &mut self,
        expected: &NadaType,
        found: &NadaType,
        found_source_ref: &SourceRef,
    ) {
        let message = format!("expected `{expected:?}`, found `{found:?}`");

        self.issues.push((vec![IssueMessage::Message(message, found_source_ref.clone())], found_source_ref.clone()));
    }

    pub(crate) fn report_invalid_operand<L: NamedElement + SourceInfo, R: NamedElement + SourceInfo>(
        &mut self,
        operation: &L,
        operand: &R,
        operand_name: &str,
        program: &ProgramMIR,
    ) -> Result<()> {
        let message1 = format!("{}'s {operand_name} operand is forbidden", operand.name());
        let message2 = format!("{} operations cannot be used as {} operands", operand.name(), operation.name());
        let note = format!("Allowed operands are: {}s and input", operation.name());

        self.issues.push((
            vec![
                IssueMessage::Message(message1, program.source_ref(operation.source_ref_index())?.clone()),
                IssueMessage::Message(message2, program.source_ref(operand.source_ref_index())?.clone()),
                IssueMessage::Note(note),
            ],
            program.source_ref(operation.source_ref_index())?.clone(),
        ));

        Ok(())
    }

    pub(crate) fn report_unexpected_type<T: TypedElement + SourceInfo>(
        &mut self,
        expected: &NadaType,
        found: &T,
        program: &ProgramMIR,
    ) -> Result<()> {
        let message = format!("expected `{expected}`, found `{:?}`", found.ty());

        self.issues.push((
            vec![IssueMessage::Message(message, program.source_ref(found.source_ref_index())?.clone())],
            program.source_ref(found.source_ref_index())?.clone(),
        ));

        Ok(())
    }

    /// Report when the type is not valid for the operation
    pub(crate) fn report_invalid_type<O: NamedElement + SourceInfo, T: TypedElement + SourceInfo>(
        &mut self,
        operation: &O,
        found: &T,
        program: &ProgramMIR,
    ) -> Result<()> {
        let message = format!("invalid type `{:?}` for operation {}", found.ty(), operation.name());

        self.issues.push((
            vec![IssueMessage::Message(message, program.source_ref(found.source_ref_index())?.clone())],
            program.source_ref(found.source_ref_index())?.clone(),
        ));

        Ok(())
    }

    pub(crate) fn report_error<M: SourceInfo>(
        &mut self,
        element: &M,
        message: &str,
        program: &ProgramMIR,
    ) -> Result<()> {
        self.issues.push((
            vec![IssueMessage::Message(message.to_string(), program.source_ref(element.source_ref_index())?.clone())],
            program.source_ref(element.source_ref_index())?.clone(),
        ));

        Ok(())
    }

    /// Returns true if the validation is successful
    pub fn is_successful(&self) -> bool {
        self.issues.is_empty()
    }

    /// Utility to print the report in human readable format
    pub fn print(&self, program: &ProgramMIR) -> Result<()> {
        for (issue_messages, source_ref) in &self.issues {
            if issue_messages.is_empty() {
                continue;
            }

            let src_id = &source_ref.file as &str;
            let start_offset = source_ref.offset as usize;
            let length = source_ref.length as usize;
            let end_offset = start_offset.checked_add(length).ok_or_else(|| anyhow!("offset overflow"))?;
            let mut report_builder = Report::build(ReportKind::Error, (src_id, start_offset..end_offset))
                .with_code(3)
                .with_message("program validation failed");

            for message in issue_messages {
                match message {
                    IssueMessage::Message(message, source_ref) => {
                        let start_offset = source_ref.offset as usize;
                        let length = source_ref.length as usize;
                        let end_offset = start_offset.checked_add(length).ok_or_else(|| anyhow!("offset overflow"))?;
                        report_builder.add_label(
                            Label::new((&source_ref.file as &str, start_offset..end_offset))
                                .with_message(message.fg(LABEL_COLOR))
                                .with_color(LABEL_COLOR),
                        );
                    }
                    IssueMessage::Note(message) => {
                        report_builder.set_note(message.fg(NOTE_COLOR));
                    }
                }
            }

            let report = report_builder.finish();
            report.eprint(self.sources_cache(program))?;
        }

        Ok(())
    }

    fn sources_cache<'a>(&self, program: &'a ProgramMIR) -> impl Cache<&'a str> {
        let sources =
            program.source_files.iter().map(|(file, content)| (file as &str, Source::from(content))).collect();
        FnCache::new((move |id| Err(Box::new(format!("Failed to fetch source '{}'", id)) as _)) as fn(&_) -> _)
            .with_sources(sources)
    }
}
