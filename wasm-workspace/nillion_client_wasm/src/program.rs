use crate::errors::{JsResult, ValueError};
use nillion_client_core::programs::{ProgramAuditorRequest, RuntimeRequirementType};
use std::collections::HashMap;
use wasm_bindgen::{prelude::wasm_bindgen, JsValue};

/// The metadata for a nada program.
#[wasm_bindgen]
pub struct ProgramMetadata {
    memory_size: u64,
    total_instructions: u64,
    instructions: HashMap<String, u64>,
    preprocessing_requirements: HashMap<String, u64>,
}

#[wasm_bindgen]
impl ProgramMetadata {
    /// Construct a program metadata out of a serialized program.
    #[wasm_bindgen(constructor)]
    pub fn new(program: &[u8]) -> JsResult<ProgramMetadata> {
        let metadata = nillion_client_core::programs::extract_program_metadata(program)
            .map_err(|e| ValueError::new_err(&format!("failed to extract program metadata: {e}")))?;
        let mut preprocessing_requirements = HashMap::new();
        for (element, count) in metadata.preprocessing_requirements {
            let key = match element {
                RuntimeRequirementType::Compare => "COMPARE",
                RuntimeRequirementType::DivisionIntegerSecret => "DIVISION_SECRET_DIVISOR",
                RuntimeRequirementType::EqualsIntegerSecret => "EQUALITY_SECRET_OUTPUT",
                RuntimeRequirementType::Modulo => "MODULO",
                RuntimeRequirementType::PublicOutputEquality => "EQUALITY_PUBLIC_OUTPUT",
                RuntimeRequirementType::TruncPr => "TRUNC_PR",
                RuntimeRequirementType::Trunc => "TRUNC",
                RuntimeRequirementType::RandomInteger => "RANDOM_INTEGER",
                RuntimeRequirementType::RandomBoolean => "RANDOM_BOOLEAN",
                RuntimeRequirementType::EcdsaAuxInfo => "ECDSA_AUX_INFO",
            };
            preprocessing_requirements.insert(key.to_string(), count as u64);
        }
        let ProgramAuditorRequest { memory_size, total_instructions, instructions, .. } = metadata;
        Ok(Self { memory_size, total_instructions, instructions, preprocessing_requirements })
    }

    /// The program memory size.
    pub fn memory_size(&self) -> u64 {
        self.memory_size
    }

    /// The total number of instructions.
    pub fn total_instructions(&self) -> u64 {
        self.total_instructions
    }

    /// The program instructions.
    pub fn instructions(&self) -> JsResult<JsValue> {
        serde_wasm_bindgen::to_value(&self.instructions.clone())
            .map_err(|e| ValueError::new_err(&format!("failed to convert instructions: {e}")))
    }

    /// The program preprocessing requirements.
    pub fn preprocessing_requirements(&self) -> JsResult<JsValue> {
        serde_wasm_bindgen::to_value(&self.preprocessing_requirements.clone())
            .map_err(|e| ValueError::new_err(&format!("failed to convert preprocessing_requirements: {e}")))
    }
}
