use super::blob::BlobService;
use crate::storage::{
    models::program::{ProgramId, ProgramModel},
    repositories::blob::BlobRepositoryError,
};
use async_trait::async_trait;
use metrics::prelude::*;
use mpc_vm::{
    protocols::MPCProtocol,
    requirements::{MPCProgramRequirements, ProgramRequirements},
    JitCompiler, MPCCompiler, Program,
};
use nada_compiler_backend::mir::ProgramMIR;
use once_cell::sync::Lazy;
use program_auditor::{ProgramAuditor, ProgramAuditorError, ProgramAuditorRequest};
use program_builder::{program_package, PackagePrograms};
use std::time::Duration;
use tracing::error;

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);
static BUILTIN_PROGRAMS: Lazy<PackagePrograms> = Lazy::new(|| program_package!("builtin"));

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait ProgramService: Send + Sync + 'static {
    async fn find(&self, program_id: &ProgramId) -> Result<Program<MPCProtocol>, BlobRepositoryError>;
    async fn upsert(&self, program_id: &ProgramId, mir: ProgramMIR) -> Result<(), UpsertProgramError>;
    fn requirements(&self, program: &Program<MPCProtocol>) -> anyhow::Result<MPCProgramRequirements>;
    fn audit(&self, request: &ProgramAuditorRequest) -> Result<(), ProgramAuditorError>;
}

pub(crate) struct DefaultProgramService {
    blob_service: Box<dyn BlobService<ProgramModel>>,
    program_auditor: ProgramAuditor,
}

impl DefaultProgramService {
    pub(crate) fn new(blob_service: Box<dyn BlobService<ProgramModel>>, program_auditor: ProgramAuditor) -> Self {
        Self { blob_service, program_auditor }
    }
}

#[async_trait]
impl ProgramService for DefaultProgramService {
    async fn find(&self, program_id: &ProgramId) -> Result<Program<MPCProtocol>, BlobRepositoryError> {
        let program = match program_id {
            ProgramId::Builtin(name) => BUILTIN_PROGRAMS.mir(name).map_err(|_| BlobRepositoryError::NotFound)?,
            ProgramId::Uploaded { .. } => self.blob_service.find_one(&program_id.to_string()).await?.mir,
        };
        match MPCCompiler::compile(program) {
            Ok(program) => Ok(program),
            Err(e) => Err(BlobRepositoryError::Internal(format!("failed to JIT compile program {program_id}: {e}"))),
        }
    }

    async fn upsert(&self, program_id: &ProgramId, mir: ProgramMIR) -> Result<(), UpsertProgramError> {
        if matches!(program_id, ProgramId::Builtin(_)) {
            return Err(UpsertProgramError::BuiltinProgram);
        }
        let model = ProgramModel { mir };
        self.blob_service.upsert(&program_id.to_string(), model).await?;
        Ok(())
    }

    fn requirements(&self, program: &Program<MPCProtocol>) -> anyhow::Result<MPCProgramRequirements> {
        let _timer = METRICS.requirements_timer();
        MPCProgramRequirements::from_program(program)
    }

    fn audit(&self, request: &ProgramAuditorRequest) -> Result<(), ProgramAuditorError> {
        match self.program_auditor.audit(request) {
            Ok(result) => Ok(result),
            Err(e) => {
                if let ProgramAuditorError::InvalidProgram(violation) = &e {
                    METRICS.inc_audit_errors(&violation.policy);
                }
                Err(e)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum UpsertProgramError {
    #[error(transparent)]
    Blob(#[from] BlobRepositoryError),

    #[error("can't upload builtin programs")]
    BuiltinProgram,
}

struct Metrics {
    requirements_duration: MaybeMetric<Histogram<Duration>>,
    audit_errors: MaybeMetric<Counter>,
}

impl Default for Metrics {
    fn default() -> Self {
        let requirements_duration = Histogram::new(
            "program_requirements_analysis_duration_seconds",
            "Duration of the analysis of a program's requirements in seconds",
            &[],
            TimingBuckets::sub_second(),
        )
        .into();
        let audit_errors = Counter::new(
            "program_audit_errors_total",
            "Number of errors detected when running program audits",
            &["policy"],
        )
        .into();
        Self { requirements_duration, audit_errors }
    }
}

impl Metrics {
    fn requirements_timer(&self) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.requirements_duration.with_labels([]).into_timer()
    }

    fn inc_audit_errors(&self, policy: &str) {
        self.audit_errors.with_labels([("policy", policy)]).inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::blob::MockBlobService;
    use rstest::rstest;
    use test_programs::PROGRAMS;

    #[tokio::test]
    async fn store_builtin_program() {
        let service =
            DefaultProgramService::new(Box::new(MockBlobService::default()), ProgramAuditor::new(Default::default()));
        let program_id = ProgramId::Builtin("foo".into());
        let program = PROGRAMS.mir("simple_shares").unwrap();
        let error = service.upsert(&program_id, program).await.expect_err("upload succeeded");
        assert!(matches!(error, UpsertProgramError::BuiltinProgram));
    }

    #[rstest]
    #[case::tecdsa_sign("tecdsa_sign")]
    fn builtin_program_lookup(#[case] name: &str) {
        BUILTIN_PROGRAMS.mir(name).expect("program not found");
        // TODO: eventually look this up once the bytecode protocol is implemented
    }
}
