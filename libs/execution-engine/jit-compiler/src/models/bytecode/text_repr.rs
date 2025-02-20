//! This library implements the text representation for the bytecode model

use anyhow::{anyhow, Error};
use substring::Substring;

use crate::models::{
    bytecode::{Input, Operation, Output, ProgramBytecode},
    SourceRef, SourceRefIndex,
};

/// formats the text representation of operations
/// it will contain the operation string and the snippet of the source code aligned at width 100 chars
fn format_text_repr(mut op_str: String, snippet: String) -> String {
    let pad = 100usize.saturating_sub(op_str.len());
    op_str.push_str(&" ".repeat(pad));
    op_str.push_str(&format!("# {snippet}"));
    op_str
}

impl ProgramBytecode {
    /// This function tries to return the snippet from a source ref if it exists
    pub fn snippet(&self, source_ref: &SourceRef) -> Result<Option<String>, Error> {
        let source_file = self.source_files.get(&source_ref.file);
        if let Some(snippet) = source_file {
            let start_offset = source_ref.offset as usize;
            let end_offset =
                start_offset.checked_add(source_ref.length as usize).ok_or_else(|| anyhow!("end offset overflow"))?;
            let snippet = snippet.substring(start_offset, end_offset).trim().to_string();
            Ok(Some(snippet))
        } else {
            Ok(None)
        }
    }

    /// This function tries to return the snippet from a source ref if it exists with file and lineno
    pub fn snippet_with_loc(&self, source_ref_index: &SourceRefIndex) -> Result<Option<String>, Error> {
        let source_ref = self.source_ref(*source_ref_index)?;
        self.snippet(source_ref)
            .map(|snippet| snippet.map(|snippet| format!("{}  -> {}:{}", snippet, source_ref.file, source_ref.lineno)))
    }

    /// Returns the text representation of the program
    pub fn text_repr(&self) -> String {
        let mut repr = self.header_text_repr();
        repr.push_str("\n\n\n");
        repr.push_str("Operations:\n");
        for operation in self.memory.heap.iter() {
            repr.push_str(&operation.text_repr(self));
            repr.push('\n');
        }
        repr
    }

    /// Returns the header text representation of the program
    pub fn header_text_repr(&self) -> String {
        let mut header_str = String::from("Header:\n");
        for (party_id, party) in self.parties.iter().enumerate() {
            let snippet = self.snippet_with_loc(&party.source_ref_index).unwrap_or_default().unwrap_or_default();
            header_str.push_str(&format!("Party: {} id({})   # {}\n", party.name, party_id, snippet));

            header_str.push_str(" Inputs:\n");
            for input in self.inputs().filter(|input| input.party_id == party_id) {
                let input_txt = input.text_repr(self);
                header_str.push_str(&format!("  {input_txt}\n"));
            }

            header_str.push_str(" Outputs:\n");
            for output in self.outputs().filter(|output| output.party_id == party_id) {
                let output_txt = output.text_repr(self);
                header_str.push_str(&format!("  {output_txt}\n"));
            }
            header_str.push('\n');
        }
        header_str.push_str("Literals:\n");
        for literal in self.literals() {
            header_str.push_str(&format!(" {literal}\n"));
        }

        header_str
    }
}

impl Output {
    /// Returns the text representation of the output
    pub fn text_repr(&self, program: &ProgramBytecode) -> String {
        let snippet = program.snippet_with_loc(&self.source_ref_index).unwrap_or_default().unwrap_or_default();
        format_text_repr(self.to_string(), snippet)
    }
}

impl Input {
    /// Returns the text representation of the input
    pub fn text_repr(&self, program: &ProgramBytecode) -> String {
        let snippet = program.snippet_with_loc(&self.source_ref_index).unwrap_or_default().unwrap_or_default();
        format_text_repr(self.to_string(), snippet)
    }
}

impl Operation {
    /// returns the text representation of the operation
    pub fn text_repr(&self, program: &ProgramBytecode) -> String {
        let source_ref = self.source_ref_index();
        let snippet = program.snippet_with_loc(source_ref).unwrap_or_default().unwrap_or_default();
        format_text_repr(self.to_string(), snippet)
    }
}

// ModelElement uses enum_dispatch, but it doesn't work if the trait is defined in a different crate.
// In this case, ModelElement is defined in the compiler-backend, for this reason, we have to implement
// the trait for bytecode::Operation. https://gitlab.com/antonok/enum_dispatch/-/issues/56
impl Operation {
    fn source_ref_index(&self) -> &SourceRefIndex {
        use Operation::*;
        match self {
            Not(op) => op.source_ref_index(),
            Addition(op) => op.source_ref_index(),
            Subtraction(op) => op.source_ref_index(),
            Multiplication(op) => op.source_ref_index(),
            Cast(op) => op.source_ref_index(),
            Load(op) => op.source_ref_index(),
            Get(op) => op.source_ref_index(),
            New(op) => op.source_ref_index(),
            Modulo(op) => op.source_ref_index(),
            Power(op) => op.source_ref_index(),
            Division(op) => op.source_ref_index(),
            LessThan(op) => op.source_ref_index(),
            PublicOutputEquality(op) => op.source_ref_index(),
            Equals(op) => op.source_ref_index(),
            LeftShift(op) => op.source_ref_index(),
            RightShift(op) => op.source_ref_index(),
            Random(op) => op.source_ref_index(),
            TruncPr(op) => op.source_ref_index(),
            Literal(op) => op.source_ref_index(),
            IfElse(op) => op.source_ref_index(),
            Reveal(op) => op.source_ref_index(),
            PublicKeyDerive(op) => op.source_ref_index(),
            InnerProduct(op) => op.source_ref_index(),
            EcdsaSign(op) => op.source_ref_index(),
            EddsaSign(op) => op.source_ref_index(),
        }
    }
}
