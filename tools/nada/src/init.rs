use crate::nada_project_toml::{NadaProjectToml, ProgramToml};
use eyre::Result;
use std::{collections::HashMap, env::current_dir, fs::create_dir};

const MAIN_PY: &str = r#"from nada_dsl import *

def nada_main():
    party1 = Party(name="Party1")
    party2 = Party(name="Party2")
    party3 = Party(name="Party3")
    a = SecretInteger(Input(name="A", party=party1))
    b = SecretInteger(Input(name="B", party=party2))

    result = a + b

    return [Output(result, "my_output", party3)]"#;

pub fn init(name: String) -> Result<()> {
    // create dir with name in current dir
    let cwd = current_dir()?;
    let project_path = cwd.join(&name);
    create_dir(&project_path)?;

    let src_path = project_path.join("src");
    create_dir(&src_path)?;

    let tests_path = project_path.join("tests");
    create_dir(tests_path)?;

    let target_path = project_path.join("target");
    create_dir(target_path)?;

    let toml_path = project_path.join("nada-project.toml");

    let program = ProgramToml {
        name: None,
        path: "src/main.py".to_string(),
        prime_size: crate::nada_project_toml::PrimeSize::Medium128bit,
    };
    let nada_project_toml = NadaProjectToml {
        name,
        version: "0.1.0".to_string(),
        authors: vec!["".to_string()],
        programs: vec![program],
        test_framework: HashMap::new(),
        networks: HashMap::default(),
    };

    let toml_str = toml::to_string(&nada_project_toml)?;

    std::fs::write(toml_path, toml_str)?;

    let main_py_path = src_path.join("main.py");
    std::fs::write(main_py_path, MAIN_PY)?;

    Ok(())
}
