//! Implementation of new-program command

use eyre::{eyre, Context, Result};
use std::env::current_dir;
use toml_edit::{value, DocumentMut, Table};

use crate::{
    nada_project_toml::{PrimeSize, ProgramToml},
    paths,
};

const TEMPLATE_PROGRAM: &str = r#"from nada_dsl import *

def nada_main():
    """This is a template, add your code here"""
"#;

fn create_new_program(name: &str) -> Result<()> {
    let src_path = current_dir()?.join("src");
    let program_path = src_path.join(format!("{name}.py"));
    std::fs::write(program_path, TEMPLATE_PROGRAM)?;

    Ok(())
}

fn add_program_to_project_config(program: ProgramToml) -> Result<()> {
    let nada_project_toml_path = paths::get_nada_project_toml_path()?;
    let nada_project_toml_str =
        std::fs::read_to_string(&nada_project_toml_path).context("Reading nada-project.toml")?;
    let mut nada_project_toml = nada_project_toml_str.parse::<DocumentMut>()?;
    let mut program_table = Table::default();
    if let Some(name) = program.name {
        program_table["name"] = value(name);
    }
    program_table["path"] = value(program.path);
    program_table["prime_size"] = value(program.prime_size as i64);
    nada_project_toml["programs"]
        .as_array_of_tables_mut()
        .ok_or(eyre!("Invalid project configuration, missing 'programs' section"))?
        .push(program_table);
    std::fs::write(nada_project_toml_path, nada_project_toml.to_string())?;
    Ok(())
}

/// Adds a new program to the project.
pub fn new_program(name: String, prime_size: Option<PrimeSize>) -> Result<()> {
    // create new program file
    create_new_program(&name)?;
    // Update nada-project.toml
    let new_program = ProgramToml {
        name: Some(name.clone()),
        path: format!("src/{name}.py"),
        prime_size: prime_size.unwrap_or(PrimeSize::Medium128bit),
    };
    add_program_to_project_config(new_program)?;
    Ok(())
}
