use anyhow::Error;
use bytecode_evaluator::Evaluator;
use jit_compiler::{mir2bytecode::MIR2Bytecode, models::bytecode::ProgramBytecode};
use log::debug;
use math_lib::modular::{SafePrime, U128SafePrime, U256SafePrime, U64SafePrime};
use nada_compiler_backend::mir::{
    proto::{ConvertProto, Message},
    ProgramMIR,
};
use nada_value::{clear::Clear, NadaValue};
use num_bigint::{BigInt, BigUint};
use serde::Deserialize;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::collections::HashMap;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::console;

/// Build [`InputGenerator`] from a string.
///
/// Example input:
/// { "my_int": {"type": "SecretInteger", "value": "6"}, "my_uint": {"type": "SecretUnsignedInteger", "value":  "7"}}
///
/// # Arguments
/// * `inputs` - The string representing the serialised version of secrets
fn build_inputs(inputs: String) -> Result<HashMap<String, NadaValue<Clear>>, JsError> {
    let inputs: HashMap<String, HashMap<String, String>> = serde_json::from_str(&inputs)?;
    console::log_1(&JsValue::from(format!("building secrets: {inputs:?}")));
    let mut inputs_map = HashMap::new();
    for (name, value) in inputs {
        let nada_value = match value["type"].as_str() {
            "SecretInteger" => NadaValue::new_secret_integer(value["value"].parse::<BigInt>()?),
            "SecretUnsignedInteger" => NadaValue::new_secret_unsigned_integer(value["value"].parse::<BigUint>()?),
            "PublicInteger" => NadaValue::new_integer(value["value"].parse::<BigInt>()?),
            "PublicUnsignedInteger" => NadaValue::new_unsigned_integer(value["value"].parse::<BigUint>()?),
            value => Err(JsError::new(&format!("{} not a valid type", value)))?,
        };
        inputs_map.insert(name, nada_value);
    }
    Ok(inputs_map)
}

fn evaluate<T>(
    bytecode: &ProgramBytecode,
    values: HashMap<String, NadaValue<Clear>>,
) -> Result<HashMap<String, NadaValue<Clear>>, Error>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let outputs = Evaluator::<T>::run(bytecode, values)?;
    Ok(outputs)
}

#[wasm_bindgen]
pub fn program_to_bin(program_json: String) -> Result<Vec<u8>, JsError> {
    let program: ProgramMIR = serde_json::from_str(&program_json)
        .map_err(|e| JsError::new(&format!("There was a problem parsing the nada program: {e}")))?;
    let program = program.into_proto().encode_to_vec();
    Ok(program)
}

#[wasm_bindgen]
pub fn run(program_json: String, prime_size: usize, inputs: String) -> Result<JsValue, JsError> {
    console::log_1(&JsValue::from_str("Running program simulator"));
    // As per wasm-bindgen recommendation for better console errors
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let mut deserializer = serde_json::Deserializer::from_str(&program_json);
    deserializer.disable_recursion_limit();
    let deserializer = serde_stacker::Deserializer::new(&mut deserializer);
    let program_mir: ProgramMIR = Deserialize::deserialize(deserializer)
        .map_err(|err| JsError::new(&format!("failed parsing program MIR: {err}\nMIR: {program_json}")))?;

    debug!("Parsing program");
    let bytecode = MIR2Bytecode::transform(&program_mir)
        .map_err(|e| JsError::new(&format!("failed to compile program's MIR: {e}")))?;

    debug!("Loading secrets");
    let inputs = build_inputs(inputs)?;

    let result = match prime_size {
        64 => {
            let result = evaluate::<U64SafePrime>(&bytecode, inputs).map_err(|e| JsError::new(&format!("{e}")))?;
            output_to_hashmap(result).map_err(|e| JsError::new(&format!("{e}")))?
        }
        128 => {
            let result = evaluate::<U128SafePrime>(&bytecode, inputs).map_err(|e| JsError::new(&format!("{e}")))?;
            output_to_hashmap(result).map_err(|e| JsError::new(&format!("{e}")))?
        }
        256 => {
            let result = evaluate::<U256SafePrime>(&bytecode, inputs).map_err(|e| JsError::new(&format!("{e}")))?;
            output_to_hashmap(result).map_err(|e| JsError::new(&format!("{e}")))?
        }
        _ => Err(JsError::new("invalid prime size"))?,
    };
    console::log_1(&JsValue::from_str(&format!("Program result: {result:?}")));
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&format!("{e}")))
}

fn output_to_hashmap(outputs: HashMap<String, NadaValue<Clear>>) -> Result<HashMap<String, String>, Error> {
    let mut js_output = HashMap::new();
    for (output_name, output) in outputs {
        js_output.insert(output_name, format!("{:?}", output));
    }
    Ok(js_output)
}

#[cfg(test)]
mod tests {

    use crate::build_inputs;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    const SECRETS: &str = "{ \"my_int\": {\"type\": \"SecretInteger\", \"value\": \"6\"}, \"my_uint\": {\"type\": \"SecretUnsignedInteger\", \"value\":  \"7\"}}";

    #[wasm_bindgen_test]
    fn test_build_secrets() {
        let _ = build_inputs(SECRETS.to_string()).map_err(JsValue::from).unwrap();
    }
}
