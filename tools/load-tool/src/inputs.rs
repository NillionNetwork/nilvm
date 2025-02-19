//! Operation input generation.

use crate::{
    clients_pool::Clients,
    flow::ComputeArgs,
    spec::{ComputeMode, ComputeSpec, ValuesSpec},
};
use anyhow::bail;
use log::info;
use mpc_vm::{JitCompiler, MPCCompiler};
use nada_compiler_backend::{
    mir::{proto::ConvertProto, ProgramMIR},
    program_contract::Input,
};
use nada_value::{clear::Clear, NadaType, NadaValue};
use rand::{distributions::Alphanumeric, Rng};
use std::{collections::HashMap, fs, path::Path};

/// A helper type to generate arguments for different flows.
pub struct ArgsGenerator;

impl ArgsGenerator {
    /// Loads secrets from a spec.
    pub fn load_secrets_spec(spec: ValuesSpec) -> anyhow::Result<HashMap<String, NadaValue<Clear>>> {
        let secrets = match spec {
            ValuesSpec::Custom { inputs } => inputs.parse_values()?.map(|input| (input.name, input.value)).collect(),
            ValuesSpec::Blob { size } => {
                let secret = NadaValue::new_secret_blob(vec![42; size.0]);
                HashMap::from([("foo".to_string(), secret)])
            }
        };
        Ok(secrets)
    }

    /// Create the compute flow arguments.
    pub async fn build_compute_args(
        base_path: &Path,
        spec: ComputeSpec,
        clients: &mut Clients,
    ) -> anyhow::Result<ComputeArgs> {
        let (program_id, raw_program_mir) = Self::upload_program(clients, base_path, &spec.program_path).await?;

        let program_mir = ProgramMIR::try_decode(&raw_program_mir)?;
        let program = MPCCompiler::compile(program_mir)?;
        let our_party_name = program.contract.input_parties()?.first().expect("no parties").name.clone();
        let inputs = program.contract.inputs_by_party_name()?;

        let test_values = if let Some(values) = spec.inputs {
            Self::load_secrets_spec(values)?
        } else {
            let Some(our_secrets) = inputs.get(&our_party_name) else { bail!("party {our_party_name} not found") };
            Self::generate_test_values(our_secrets)?
        };

        let (store_values, compute_values) = match spec.mode {
            ComputeMode::StoreNone => (Default::default(), test_values),
            ComputeMode::StoreAll => (test_values, Default::default()),
            ComputeMode::StoreHalf => {
                let total_secrets = test_values.len();
                let mut values = test_values.into_iter();
                let store_values: HashMap<String, NadaValue<Clear>> = values.by_ref().take(total_secrets / 2).collect();
                let compute_values = values.collect();
                (store_values, compute_values)
            }
        };
        let values_ids = match store_values.is_empty() {
            true => vec![],
            false => {
                info!("Storing {} secrets...", store_values.len());
                let value_id = clients
                    .vm
                    .store_values()
                    .ttl_days(1)
                    .add_values(store_values.clone())
                    .allow_compute(clients.vm.user_id(), program_id.clone())
                    .build()?
                    .invoke()
                    .await?;
                vec![value_id]
            }
        };
        let args = ComputeArgs {
            program_id,
            store_ids: values_ids,
            values: compute_values,
            input_parties: program.contract.input_parties()?.iter().map(|party| party.name.clone()).collect(),
            output_parties: program.contract.output_parties()?.iter().map(|party| party.name.clone()).collect(),
        };
        Ok(args)
    }

    /// Upload a program.
    pub async fn upload_program(
        clients: &mut Clients,
        base_path: &Path,
        program_path: &Path,
    ) -> anyhow::Result<(String, Vec<u8>)> {
        let program_path = match program_path.is_absolute() {
            true => program_path.into(),
            false => base_path.join(program_path),
        };
        let raw_program_mir = fs::read(program_path)?;
        let program_name = rand::thread_rng().sample_iter(Alphanumeric).take(32).map(char::from).collect::<String>();
        info!("Uploading program {program_name}...");
        let program_id = clients
            .vm
            .store_program()
            .name(program_name.clone())
            .program(raw_program_mir.clone())
            .build()?
            .invoke()
            .await?;
        Ok((program_id, raw_program_mir))
    }

    fn generate_test_value(nada_type: &NadaType) -> anyhow::Result<NadaValue<Clear>> {
        use NadaType::*;
        let secret = match nada_type {
            SecretInteger => NadaValue::new_secret_integer(42),
            SecretUnsignedInteger => NadaValue::new_secret_unsigned_integer(42u32),
            Integer => NadaValue::new_integer(42),
            UnsignedInteger => NadaValue::new_unsigned_integer(42u32),
            SecretBlob
            | EcdsaDigestMessage
            | ShamirShareInteger
            | ShamirShareUnsignedInteger
            | ShamirShareBoolean
            | Boolean
            | SecretBoolean
            | Tuple { .. }
            | Array { .. }
            | NTuple { .. }
            | Object { .. }
            | EcdsaPrivateKey
            | EcdsaSignature
            | EcdsaPublicKey
            | EddsaPrivateKey
            | EddsaPublicKey
            | EddsaSignature
            | EddsaMessage
            | StoreId => bail!("type not supported {:?}", nada_type),
        };
        Ok(secret)
    }

    fn generate_test_values(inputs: &[&Input]) -> anyhow::Result<HashMap<String, NadaValue<Clear>>> {
        let mut values = HashMap::new();
        for input in inputs {
            values.insert(input.name.clone(), Self::generate_test_value(&input.ty)?);
        }
        Ok(values)
    }
}
