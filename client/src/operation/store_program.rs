//! Store program operation.

use super::{BuildError, CollapseResult, InvokeError, PaidOperation, PaidVmOperation};
use crate::{grpc::ProgramsClient, retry::Retrier, vm::VmClient};
use nillion_client_core::programs::{MPCProgramRequirements, RuntimeRequirementType};
use node_api::{
    payments::rust::{
        AuxiliaryMaterialRequirement, PreprocessingRequirement, PriceQuoteRequest, ProgramMetadata, SignedReceipt,
        StoreProgram,
    },
    preprocessing::rust::{AuxiliaryMaterial, PreprocessingElement},
    programs::rust::StoreProgramRequest,
};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, mem};
use tonic::async_trait;

static PROGRAM_ALPHABET: Lazy<HashSet<char>> =
    Lazy::new(|| ('a'..='z').chain('A'..='Z').chain('0'..='9').chain("+.:_-".chars()).collect());
const MAX_PROGRAM_NAME_LENGTH: usize = 128;

/// A preprocessing pool status operation.
pub struct StoreProgramOperation {
    program: Vec<u8>,
    operation: StoreProgram,
}

#[async_trait]
impl PaidVmOperation for StoreProgramOperation {
    type Output = String;

    const NAME: &str = "store-program";

    fn price_quote_request(&self) -> PriceQuoteRequest {
        PriceQuoteRequest::StoreProgram(self.operation.clone())
    }

    async fn invoke(mut self, vm: &VmClient, signed_receipt: SignedReceipt) -> Result<Self::Output, InvokeError> {
        let mut retrier = Retrier::default();
        let request = StoreProgramRequest { program: mem::take(&mut self.program), signed_receipt };
        for (party, clients) in &vm.clients {
            retrier.add_request(party.clone(), &clients.programs, request.clone());
        }
        let results = retrier.invoke(ProgramsClient::store_program).await;
        results.collapse_default()
    }
}

/// A builder for a store program operation.
///
/// See [PaidOperation] for more information.
#[must_use]
pub struct StoreProgramOperationBuilder<'a> {
    client: &'a VmClient,
    name: Option<String>,
    program: Option<Vec<u8>>,
}

impl<'a> StoreProgramOperationBuilder<'a> {
    pub(crate) fn new(client: &'a VmClient) -> Self {
        Self { client, name: None, program: None }
    }

    /// Set the name for the program.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the contents of the compiled program.
    pub fn program(mut self, program: Vec<u8>) -> Self {
        self.program = Some(program);
        self
    }

    /// Build this operation.
    pub fn build(mut self) -> Result<PaidOperation<'a, StoreProgramOperation>, BuildError> {
        let name = self.name.take().ok_or_else(|| BuildError("'name' not set".into()))?;
        let program = self.program.take().ok_or_else(|| BuildError("'program' not set".into()))?;

        if name.is_empty() {
            return Err(BuildError("empty program names are not allowed".into()));
        }
        if name.len() > MAX_PROGRAM_NAME_LENGTH {
            return Err(BuildError("name is too long".into()));
        }
        for c in name.chars() {
            if !PROGRAM_ALPHABET.contains(&c) {
                return Err(BuildError(format!("'{c}' is not allowed in program names")));
            }
        }

        let contents_sha256 = Sha256::digest(&program).to_vec();
        let metadata = nillion_client_core::programs::extract_program_metadata(&program)
            .map_err(|e| BuildError(format!("failed to extract program metadata: {e}")))?;
        let (preprocessing_requirements, auxiliary_material_requirements) =
            Self::translate_program_requirements(metadata.preprocessing_requirements);
        let metadata = ProgramMetadata {
            program_size: program.len() as u64,
            memory_size: metadata.memory_size,
            instruction_count: metadata.total_instructions,
            instructions: metadata.instructions,
            preprocessing_requirements,
            auxiliary_material_requirements,
        };
        let operation = StoreProgramOperation { program, operation: StoreProgram { metadata, contents_sha256, name } };
        Ok(PaidOperation::new(operation, self.client))
    }

    fn translate_program_requirements(
        requirements: MPCProgramRequirements,
    ) -> (Vec<PreprocessingRequirement>, Vec<AuxiliaryMaterialRequirement>) {
        let mut preprocessing = Vec::new();
        let mut auxiliary_material = Vec::new();

        for (element, count) in requirements {
            let element = match element {
                RuntimeRequirementType::Compare => PreprocessingElement::Compare,
                RuntimeRequirementType::DivisionIntegerSecret => PreprocessingElement::DivisionSecretDivisor,
                RuntimeRequirementType::EqualsIntegerSecret => PreprocessingElement::EqualitySecretOutput,
                RuntimeRequirementType::Modulo => PreprocessingElement::Modulo,
                RuntimeRequirementType::PublicOutputEquality => PreprocessingElement::EqualityPublicOutput,
                RuntimeRequirementType::TruncPr => PreprocessingElement::TruncPr,
                RuntimeRequirementType::Trunc => PreprocessingElement::Trunc,
                RuntimeRequirementType::RandomInteger => PreprocessingElement::RandomInteger,
                RuntimeRequirementType::RandomBoolean => PreprocessingElement::RandomBoolean,
                RuntimeRequirementType::EcdsaAuxInfo => {
                    auxiliary_material.push(AuxiliaryMaterialRequirement {
                        material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo,
                        version: 0,
                    });
                    continue;
                }
            };
            preprocessing.push(PreprocessingRequirement { element, count: count as u64 });
        }
        (preprocessing, auxiliary_material)
    }
}
