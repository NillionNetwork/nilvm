use crate::{
    controllers::{InvalidReceiptType, TraceRequest},
    services::{
        programs::{ProgramService, UpsertProgramError},
        receipts::ReceiptsService,
    },
    storage::models::program::ProgramId,
};
use async_trait::async_trait;
use grpc_channel::auth::AuthenticateRequest;
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use node_api::{
    payments::rust::{OperationMetadata, StoreProgram},
    programs::{
        proto,
        rust::{StoreProgramRequest, StoreProgramResponse},
    },
    TryIntoRust,
};
use once_cell::sync::Lazy;
use program_auditor::{ProgramAuditorError, ProgramAuditorRequest};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};
use tracing::{info, instrument};

static PROGRAM_ALPHABET: Lazy<HashSet<char>> =
    Lazy::new(|| ('a'..='z').chain('A'..='Z').chain('0'..='9').chain("+.:_-".chars()).collect());
const MAX_PROGRAM_NAME_LENGTH: usize = 128;

pub(crate) struct ProgramsApiServices {
    pub(crate) programs: Arc<dyn ProgramService>,
    pub(crate) receipts: Arc<dyn ReceiptsService>,
}

/// The programs API.
pub(crate) struct ProgramsApi {
    services: ProgramsApiServices,
}

impl ProgramsApi {
    /// Construct a new programs service.
    pub(crate) fn new(services: ProgramsApiServices) -> Self {
        Self { services }
    }

    fn validate_hash(contents: &[u8], expected_hash: &[u8]) -> tonic::Result<()> {
        let hash = Sha256::digest(contents);
        if hash.as_slice() == expected_hash { Ok(()) } else { Err(Status::invalid_argument("invalid program hash")) }
    }

    fn validate_name(name: &str) -> tonic::Result<()> {
        if name.is_empty() {
            return Err(Status::invalid_argument("empty program names are not allowed"));
        }
        if name.len() > MAX_PROGRAM_NAME_LENGTH {
            return Err(Status::invalid_argument("program name is too long"));
        }
        for c in name.chars() {
            if !PROGRAM_ALPHABET.contains(&c) {
                return Err(Status::invalid_argument(format!("'{c}' is not allowed in program names")));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl proto::programs_server::Programs for ProgramsApi {
    #[instrument(name = "api.programs.store_program", skip_all, fields(user_id = request.trace_user_id()))]
    async fn store_program(
        &self,
        request: Request<proto::store::StoreProgramRequest>,
    ) -> tonic::Result<Response<proto::store::StoreProgramResponse>> {
        let user_id = request.user_id()?;
        let request: StoreProgramRequest = request.into_inner().try_into_rust()?;
        let receipt = self.services.receipts.verify_payment_receipt(request.signed_receipt).await?;
        let OperationMetadata::StoreProgram(metadata) = receipt.metadata else {
            return Err(InvalidReceiptType("store program").into());
        };

        // TODO: eventually validate that metadata matches
        let StoreProgram { contents_sha256, name, .. } = metadata;
        Self::validate_name(&name)?;
        Self::validate_hash(&request.program, &contents_sha256)?;

        let program = ProgramMIR::try_decode(&request.program)
            .map_err(|_| Status::invalid_argument("malformed program (invalid sdk version?)"))?;

        let program_id = ProgramId::Uploaded { user_id, name, sha256: contents_sha256 };
        let request = ProgramAuditorRequest::from_mir(&program)
            .map_err(|e| Status::invalid_argument(format!("invalid program: {e}")))?;
        match self.services.programs.audit(&request) {
            Ok(_) => {
                info!("Storing program with id {program_id}");
                self.services.programs.upsert(&program_id, program).await?;
                Ok(Response::new(StoreProgramResponse { program_id: program_id.to_string() }))
            }
            Err(ProgramAuditorError::Unexpected(e)) => Err(Status::internal(format!("failed to audit program: {e}"))),
            Err(e) => Err(Status::invalid_argument(e.to_string())),
        }
    }
}

impl From<UpsertProgramError> for Status {
    fn from(e: UpsertProgramError) -> Self {
        use UpsertProgramError::*;
        match e {
            Blob(e) => e.into(),
            BuiltinProgram => Self::invalid_argument(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controllers::tests::{empty_signed_receipt, MakeAuthenticated, ReceiptBuilder},
        services::{programs::MockProgramService, receipts::MockReceiptsService},
    };
    use mockall::predicate::always;
    use node_api::{auth::rust::UserId, payments::rust::ProgramMetadata, ConvertProto};
    use proto::programs_server::Programs;
    use std::{cell::RefCell, rc::Rc};
    use test_programs::PROGRAMS;

    #[derive(Default)]
    struct ServiceBuilder {
        programs: MockProgramService,
        receipts: MockReceiptsService,
    }

    impl ServiceBuilder {
        fn build(self) -> ProgramsApi {
            ProgramsApi::new(ProgramsApiServices {
                programs: Arc::new(self.programs),
                receipts: Arc::new(self.receipts),
            })
        }
    }

    #[tokio::test]
    async fn store_program() {
        let program = PROGRAMS.metadata("simple").unwrap().raw_mir();
        let user_id = UserId::from_bytes("bob");
        let receipt = ReceiptBuilder::new(StoreProgram {
            metadata: ProgramMetadata {
                program_size: 0,
                memory_size: 0,
                instruction_count: 0,
                instructions: Default::default(),
                preprocessing_requirements: Default::default(),
                auxiliary_material_requirements: Default::default(),
            },
            contents_sha256: Sha256::digest(&program).to_vec(),
            name: "test".into(),
        })
        .build();
        let mut builder = ServiceBuilder::default();
        builder.receipts.expect_verify_payment_receipt().return_once(move |_| Ok(receipt));
        builder.programs.expect_audit().return_once(|_| Ok(()));

        // Save the program id so we can ensure we're returned the same id we used in storage
        let program_id = Rc::new(RefCell::new(None));
        {
            let program_id = program_id.clone();
            builder.programs.expect_upsert().with(always(), always()).returning_st(move |id, _| {
                *program_id.borrow_mut() = Some(id.clone());
                Ok(())
            });
        }

        let api = builder.build();
        let request =
            Request::new(StoreProgramRequest { program, signed_receipt: empty_signed_receipt() }.into_proto())
                .authenticated(user_id);
        let response = api.store_program(request).await.expect("failed to store").into_inner();
        assert_eq!(response.program_id, *program_id.borrow().as_ref().unwrap().to_string());
    }

    #[test]
    fn name_validation() {
        ProgramsApi::validate_name("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890+.:_-")
            .expect("invalid name");

        ProgramsApi::validate_name("/").expect_err("valid name");
        ProgramsApi::validate_name("").expect_err("valid name");
        ProgramsApi::validate_name(&['a'; MAX_PROGRAM_NAME_LENGTH + 1].iter().collect::<String>())
            .expect_err("valid name");
    }
}
