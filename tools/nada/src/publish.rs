use eyre::{eyre, Result};
use nillion_client::vm::VmClient;
use rand::distributions::{Alphanumeric, DistString};
use std::fs;
use tools_config::client::ClientParameters;

use crate::{
    error::IntoEyre,
    nada_project_toml::{NadaProjectToml, ProgramConf},
    paths::get_compiled_program_path,
};
const RAND_CHARS: usize = 8;

/// Appends a random string of characters to the program name.
/// This is done so that the user can upload several versions of the program.
fn randomize_program_name(program_name: &str) -> String {
    let random_str = Alphanumeric.sample_string(&mut rand::thread_rng(), RAND_CHARS);
    format!("{program_name}-{random_str}")
}

pub(crate) async fn publish_program(network: &String, program_conf: &ProgramConf) -> Result<(String, VmClient)> {
    let conf = NadaProjectToml::find_self()?;
    let Some(network_conf) = conf.networks.get(network) else {
        return Err(eyre!("Network {network} not found in the project configuration"));
    };
    // Create a nillion client using identities and network configuration
    let parameters = ClientParameters { identity: network_conf.identity.clone(), network: network.clone() };
    let client = parameters.try_build().await.into_eyre()?;
    let path = get_compiled_program_path(program_conf)?;
    let name = randomize_program_name(&program_conf.name);
    let body = fs::read(&path).map_err(|e| eyre!("{e:?}"))?;
    let program_id = client.store_program().name(name).program(body).build()?.invoke().await.into_eyre()?;
    Ok((program_id, client))
}
