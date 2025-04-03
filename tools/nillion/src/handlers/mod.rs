use crate::serialize::SerializeAsAny;
use anyhow::{anyhow, Result};
use std::{env, path::Path};

pub mod context;
pub mod identities;
pub mod networks;
pub mod nilauth;
pub mod nilvm;
pub mod nuc;

pub type HandlerResult = Result<Box<dyn SerializeAsAny>>;

pub(crate) fn open_in_editor(path: &Path) -> Result<()> {
    // Use the editor specified in VISUAL, otherwise EDITOR, otherwise default to vim.
    let editor = env::var("VISUAL").or_else(|_| env::var("EDITOR")).unwrap_or_else(|_| "vim".into());
    let mut child =
        std::process::Command::new(&editor).arg(path).spawn().map_err(|e| anyhow!("failed to run {editor}: {e}"))?;
    child.wait().map_err(|e| anyhow!("failed to wait for {editor}: {e}"))?;
    Ok(())
}
