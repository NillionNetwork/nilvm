//! This create implements the models used by the jit compiler.

use nada_compiler_backend::mir::{
    Party as MIRParty, SourceFiles as MIRSourceFiles, SourceRef as MIRSourceRef, SourceRefIndex as MIRSourceRefIndex,
};
#[cfg(feature = "text_repr")]
use std::collections::BTreeMap;
#[cfg(feature = "text_repr")]
use std::ops::Deref;

pub mod bytecode;
pub mod memory;
pub mod protocols;

/// Party type that is used to identify who is the owner of an input.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Party {
    /// Name of the party
    pub name: String,
    /// Source reference
    pub source_ref_index: SourceRefIndex,
}

impl From<&MIRParty> for Party {
    fn from(mir_party: &MIRParty) -> Self {
        Self { name: mir_party.name.clone(), source_ref_index: (&mir_party.source_ref_index).into() }
    }
}

/// Sources Files contains all used files and the content of them
#[cfg(feature = "text_repr")]
#[derive(Debug, Clone, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceFiles(BTreeMap<String, String>);

#[cfg(feature = "text_repr")]
impl Deref for SourceFiles {
    type Target = BTreeMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "text_repr")]
impl From<&MIRSourceFiles> for SourceFiles {
    fn from(value: &MIRSourceFiles) -> Self {
        SourceFiles(value.clone())
    }
}

#[cfg(not(feature = "text_repr"))]
/// Sources Files contains all used files and the content of them
#[derive(Debug, Clone, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceFiles {}

#[cfg(not(feature = "text_repr"))]
impl From<&MIRSourceFiles> for SourceFiles {
    fn from(_: &MIRSourceFiles) -> Self {
        SourceFiles::default()
    }
}

/// Information about Nada-lang file that contains an element
#[cfg(feature = "text_repr")]
#[derive(Debug, Clone, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceRef {
    /// Nada-lang file that contains the elements
    pub file: String,
    /// Line number into the file that contains the element
    pub lineno: u32,
    /// Element's offset into the file
    pub offset: u32,
    /// Element's length into the file
    pub length: u32,
}

#[cfg(feature = "text_repr")]
impl From<&MIRSourceRef> for SourceRef {
    fn from(mir_source_ref: &MIRSourceRef) -> Self {
        Self {
            file: mir_source_ref.file.clone(),
            lineno: mir_source_ref.lineno,
            offset: mir_source_ref.offset,
            length: mir_source_ref.length,
        }
    }
}

#[cfg(not(feature = "text_repr"))]
/// Information about Nada-lang file that contains an element
#[derive(Debug, Clone, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceRef {}

#[cfg(not(feature = "text_repr"))]
impl From<&MIRSourceRef> for SourceRef {
    fn from(_: &MIRSourceRef) -> Self {
        Self::default()
    }
}

#[macro_export]
/// Implements SourceInfo trait
macro_rules! source_info {
    ($name:ident) => {
        impl $name {
            /// Allow access to the source information of the element
            pub fn source_ref_index(&self) -> &SourceRefIndex {
                &self.source_ref_index
            }
        }
    };
}

#[cfg(feature = "text_repr")]
pub(crate) mod text_repr_utils {
    use crate::models::{SourceFiles, SourceRef};
    use anyhow::{anyhow, Error};
    use substring::Substring;

    use super::SourceRefIndex;

    /// formats the text representation of operations
    /// it will contain the operation string and the snippet of the source code aligned at width 100 chars
    pub(crate) fn format_text_repr(mut op_str: String, reads: usize, snippet: String) -> String {
        let pad = 100usize.saturating_sub(op_str.len());
        op_str.push_str(&format!(", reads: {reads}"));
        op_str.push_str(&" ".repeat(pad));
        op_str.push_str(&format!("# {snippet}"));
        op_str
    }

    /// This function tries to return the snippet from a source ref if it exists
    pub fn snippet(source_files: &SourceFiles, source_ref: &SourceRef) -> Result<Option<String>, Error> {
        let source_file = source_files.get(&source_ref.file);
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
    pub fn snippet_with_loc(
        source_files: &SourceFiles,
        source_refs: &[SourceRef],
        source_ref_index: &SourceRefIndex,
    ) -> Result<Option<String>, Error> {
        let source_ref = source_refs
            .get(source_ref_index.0 as usize)
            .ok_or(anyhow!("source ref with index {} not found", source_ref_index.0))?;
        snippet(source_files, source_ref)
            .map(|snippet| snippet.map(|snippet| format!("{}  -> {}:{}", snippet, source_ref.file, source_ref.lineno)))
    }
}

/// Index to a source ref
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SourceRefIndex(u64);

impl From<&MIRSourceRefIndex> for SourceRefIndex {
    fn from(value: &MIRSourceRefIndex) -> Self {
        SourceRefIndex(value.0)
    }
}
