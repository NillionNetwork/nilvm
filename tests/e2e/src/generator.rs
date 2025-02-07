use anyhow::Result;
use serde_files_utils::string::write_string;
use std::{env::current_dir, path::PathBuf};
use tera::{Context, Tera};
use xshell::Shell;

const E2E_TESTS_ROOT: &str = "tests/e2e";

/// Read a bytecode model from the test repository
pub fn get_cwd() -> PathBuf {
    let mut cwd = current_dir().expect("failed to get cwd");
    // This is to allow debugging with rust analyzer because of a bug
    // see https://github.com/rust-lang/rust-analyzer/issues/13208
    if !cwd.ends_with("e2e") {
        cwd.push(E2E_TESTS_ROOT.to_string());
    }
    cwd
}

pub(crate) fn generate_program(template_name: &str, program_id: &str, context: &Context) -> Result<PathBuf> {
    let mut templates = Tera::default();
    let base_dir = get_cwd().join("templates");
    let template_path = base_dir.join(format!("{template_name}.py.template"));
    templates.add_template_file(template_path, Some(template_name))?;
    let base_dir = base_dir.join("programs");
    let sh = Shell::new()?;
    sh.create_dir(&base_dir)?;
    let program_path = base_dir.join(format!("{program_id}.py"));
    let program_content = templates.render(template_name, context)?;
    write_string(&program_path, program_content)?;
    Ok(program_path)
}
