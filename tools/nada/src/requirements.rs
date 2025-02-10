use crate::{nada_project_toml::ProgramConf, program::get_program_mir};
use eyre::{Context, Result};
use program_auditor::ProgramAuditorRequest;

pub(crate) fn program_requirements(program_conf: ProgramConf) -> Result<ProgramAuditorRequest> {
    let program_mir = get_program_mir(&program_conf)?;
    ProgramAuditorRequest::from_mir(&program_mir).with_context(|| "failed to analyze requirements of program")
}
