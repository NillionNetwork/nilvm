use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::{io::Write, process::Command};
use tempfile::NamedTempFile;
use test_programs::PROGRAMS;

fn load_program(name: &str) -> std::io::Result<NamedTempFile> {
    let mut file = tempfile::NamedTempFile::new()?;
    let contents = PROGRAMS.metadata(name).expect("program not found").raw_mir();
    file.write_all(&contents)?;
    file.flush()?;
    Ok(file)
}

#[test]
fn addition_simple_public_public() -> Result<(), Box<dyn std::error::Error>> {
    // get the command to run the nada-run binary
    let mut cmd = Command::cargo_bin("nada-run")?;
    // load the program binary
    let file = load_program("addition_simple_public_public")?;

    // command being tested:
    // nada-run --prime-size 128 --public-integer my_int1=23 --public-integer my_int2=34 addition_simple_public_public.nada.bin
    // the arguments map 1:1 to the command above
    cmd.arg("--prime-size")
        .arg("128")
        .arg("--public-integer")
        .arg("public_my_int1=23")
        .arg("--public-integer")
        .arg("public_my_int2=34")
        .arg(file.path());
    // expected result of the command when passed with the above arguments
    cmd.assert().success().stdout(predicate::str::contains("Output (my_output): Integer(NadaInt(57))"));

    Ok(())
}

#[test]
fn addition_simple() -> Result<(), Box<dyn std::error::Error>> {
    // get the command to run the nada-run binary
    let mut cmd = Command::cargo_bin("nada-run")?;
    // load the program binary
    let file = load_program("addition_simple")?;

    // command being tested:
    // nada-run --prime-size 128 --secret-integer my_int1=23 --secret-integer my_int2=34 addition_simple.nada.bin
    // the arguments map 1:1 to the command above
    cmd.arg("--prime-size")
        .arg("128")
        .arg("--secret-integer")
        .arg("my_int1=23")
        .arg("--secret-integer")
        .arg("my_int2=34")
        .arg(file.path());
    // expected result of the command when passed with the above arguments
    cmd.assert().success().stdout(predicate::str::contains("Output (my_output): SecretInteger(NadaInt(57))"));

    Ok(())
}

#[test]
fn map_simple() -> Result<(), Box<dyn std::error::Error>> {
    // get the command to run the nada-run binary
    let mut cmd = Command::cargo_bin("nada-run")?;
    // load the program binary
    let file = load_program("map_simple")?;

    cmd.arg("--prime-size")
        .arg("128")
        .arg("--array-secret-integer")
        .arg("my_array_1=1,2,3")
        .arg("--secret-integer")
        .arg("my_int=1")
        .arg(file.path());
    // expected result of the command when passed with the above arguments
    cmd.assert().success().stdout(predicate::str::contains("Output (my_output): Array { inner_type: SecretInteger, values: [SecretInteger(NadaInt(2)), SecretInteger(NadaInt(3)), SecretInteger(NadaInt(4))] }"));

    Ok(())
}

#[test]
fn map_simple_unsigned() -> Result<(), Box<dyn std::error::Error>> {
    // get the command to run the nada-run binary
    let mut cmd = Command::cargo_bin("nada-run")?;
    // load the program binary
    let file = load_program("map_simple_unsigned")?;

    cmd.arg("--prime-size")
        .arg("128")
        .arg("--array-secret-unsigned-integer")
        .arg("my_array_1=1,2,3")
        .arg("--secret-unsigned-integer")
        .arg("my_int=1")
        .arg(file.path());
    // expected result of the command when passed with the above arguments
    cmd.assert().success().stdout(predicate::str::contains("Output (my_output): Array { inner_type: SecretUnsignedInteger, values: [SecretUnsignedInteger(NadaUint(2)), SecretUnsignedInteger(NadaUint(3)), SecretUnsignedInteger(NadaUint(4))] }"));

    Ok(())
}

#[test]
fn map_simple_public() -> Result<(), Box<dyn std::error::Error>> {
    // get the command to run the nada-run binary
    let mut cmd = Command::cargo_bin("nada-run")?;
    // load the program binary
    let file = load_program("map_simple_public")?;

    // command being tested:
    // nada-run --prime-size 128 --array-public-integer my_int1=23 --secret-integer my_int2=34 map_simple_public.nada.bin
    // the arguments map 1:1 to the command above
    cmd.arg("--prime-size")
        .arg("128")
        .arg("--array-public-integer")
        .arg("my_array_1=1,2,3")
        .arg("--secret-integer")
        .arg("my_int=1")
        .arg(file.path());
    // expected result of the command when passed with the above arguments
    cmd.assert().success().stdout(predicate::str::contains("Output (my_output): Array { inner_type: SecretInteger, values: [SecretInteger(NadaInt(2)), SecretInteger(NadaInt(3)), SecretInteger(NadaInt(4))] }"));

    Ok(())
}
