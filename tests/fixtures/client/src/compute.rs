use bytecode_evaluator::EvaluatorRunner;
use math_lib::modular::EncodedModulo;
use mpc_vm::{protocols::MPCProtocol, vm::simulator::InputGenerator, Program, ProgramBytecode};
use nada_value::{clear::Clear, NadaValue};
use nillion_client::{grpc::membership::Prime, vm::VmClient, UserId};
use nodes_fixtures::nodes::Nodes;
use std::collections::HashMap;
use tracing::info;
use uuid::Uuid;

const RANDOM_SEED: [u8; 32] = *b"92959a96fd69146c5fe7cbde6e5720f2";
const MAX_ATTEMPTS: usize = 10;

#[derive(Default)]
pub struct ComputeValidatorBuilder {
    program_id: Option<String>,
    program: Option<(Program<MPCProtocol>, ProgramBytecode)>,
    clients_mode: ClientsMode,
    seed: Option<[u8; 32]>,
    invoker_client: Option<VmClient>,
}

impl ComputeValidatorBuilder {
    pub fn program_id<S: Into<String>>(mut self, program_id: S) -> Self {
        self.program_id = Some(program_id.into());
        self
    }

    pub fn program(mut self, program: Program<MPCProtocol>, bytecode: ProgramBytecode) -> Self {
        self.program = Some((program, bytecode));
        self
    }

    pub fn seed(mut self, seed: [u8; 32]) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn randomized_seed(mut self) -> Self {
        self.seed = Some(rand::random());
        self
    }

    pub fn clients_mode(mut self, mode: ClientsMode) -> Self {
        self.clients_mode = mode;
        self
    }

    pub fn invoker_client(mut self, client: VmClient) -> Self {
        self.invoker_client = Some(client);
        self
    }

    pub async fn run(self, nodes: &Nodes) {
        let program_id = self.program_id.expect("no program id");
        let (program, bytecode) = self.program.expect("no program");
        let seed = self.seed.unwrap_or(RANDOM_SEED);
        let clients_mode = self.clients_mode;
        let client = match self.invoker_client {
            Some(client) => client,
            None => nodes.build_client().await,
        };
        let cluster = client.cluster();
        let prime = match cluster.prime {
            Prime::Safe64Bits => EncodedModulo::U64SafePrime,
            Prime::Safe128Bits => EncodedModulo::U128SafePrime,
            Prime::Safe256Bits => EncodedModulo::U256SafePrime,
        };
        let evaluator = Box::<dyn EvaluatorRunner>::try_from(&prime).expect("failed to build evaluator");
        let generator = InputGenerator::new_random_prng(seed);

        let mut attempts = 0;
        info!("Using seed {seed:?}");
        let (party_inputs, expected_outputs) = loop {
            attempts += 1;
            if attempts >= MAX_ATTEMPTS {
                panic!("could not generate valid inputs in {MAX_ATTEMPTS}");
            }

            let party_inputs = Self::generate_inputs(&program_id, &program, &generator);
            let all_inputs = party_inputs.clone().into_values().flatten().collect();
            match evaluator.run(&bytecode, all_inputs) {
                Ok(outputs) => break (party_inputs, outputs),
                Err(e) if e.to_string().contains("division by zero") => {
                    info!("Input generation failed: {e}");
                    continue;
                }
                Err(e) => panic!("bytecode evaluation failed: {e}"),
            };
        };
        let party_outputs = program
            .contract
            .outputs_by_party_name()
            .expect("invalid contract")
            .into_iter()
            .map(|(party, outputs)| (party.clone(), outputs.into_iter().map(|o| o.name.clone()).collect()))
            .collect();
        let validator = ComputeValidator { program_id, party_inputs, party_outputs, expected_outputs, clients_mode };
        validator.run(nodes, client).await;
    }

    fn generate_inputs(
        program_id: &str,
        program: &Program<MPCProtocol>,
        generator: &InputGenerator,
    ) -> HashMap<String, HashMap<String, NadaValue<Clear>>> {
        let mut party_inputs = HashMap::default();
        for (party, inputs) in program.contract.inputs_by_party_name().expect("invalid contract") {
            let mut values = HashMap::new();
            for input in inputs {
                let value = generator.create(&input.name, input.ty.clone()).expect("failed to create input");
                info!("Assigning value {value:?} to input {} for program {program_id}", input.name);
                values.insert(input.name.clone(), value);
            }
            party_inputs.insert(party.clone(), values);
        }
        party_inputs
    }
}

/// A helper type to run computations and validate them.
pub struct ComputeValidator {
    party_inputs: HashMap<String, HashMap<String, NadaValue<Clear>>>,
    party_outputs: HashMap<String, Vec<String>>,
    expected_outputs: HashMap<String, NadaValue<Clear>>,
    clients_mode: ClientsMode,
    program_id: String,
}

impl ComputeValidator {
    pub fn builder() -> ComputeValidatorBuilder {
        ComputeValidatorBuilder::default()
    }

    /// TODO remove
    /// Set the clients mode to use.
    pub fn with_clients_mode(mut self, mode: ClientsMode) -> Self {
        self.clients_mode = mode;
        self
    }

    async fn run(self, nodes: &Nodes, invoker_client: VmClient) {
        let spec = self.build_spec(nodes, &invoker_client).await;
        let mut builder = invoker_client
            .invoke_compute()
            .program_id(self.program_id)
            .add_values(spec.compute_inputs)
            .add_value_ids(spec.value_ids);
        for (party, user_id) in spec.input_bindings {
            builder = builder.bind_input_party(party, user_id);
        }
        for (party, user_id) in spec.output_bindings {
            builder = builder.bind_output_party(party, [user_id]);
        }
        let operation = builder.build().expect("failed to build compute operation");
        let compute_id = operation.invoke().await.expect("failed to invoke compute");
        let mut all_outputs = HashMap::new();
        for client in spec.output_clients {
            let outputs = Self::fetch_outputs(&client, compute_id).await;
            for (name, value) in outputs {
                if all_outputs.insert(name.clone(), value).is_some() {
                    panic!("received duplicate output for value {name}");
                }
            }
        }
        assert_eq!(all_outputs, self.expected_outputs, "outputs (left) don't match expectations (right)");
    }

    async fn build_spec(&self, nodes: &Nodes, invoker_client: &VmClient) -> ExecutionSpec {
        match self.clients_mode {
            ClientsMode::Single => {
                // we are everyone
                let input_parties: Vec<_> = self.party_inputs.keys().cloned().collect();
                let input_bindings: Vec<_> =
                    input_parties.iter().map(|party| (party.clone(), invoker_client.user_id())).collect();
                let output_bindings: Vec<_> =
                    self.party_outputs.keys().cloned().map(|party| (party, invoker_client.user_id())).collect();
                let compute_inputs = self.party_inputs.clone().into_values().flatten().collect();
                ExecutionSpec {
                    value_ids: Vec::new(),
                    input_bindings,
                    output_bindings,
                    output_clients: vec![invoker_client.clone()],
                    compute_inputs,
                }
            }
            ClientsMode::OnePerParty => {
                let mut value_ids = Vec::new();
                let mut input_bindings = Vec::new();
                let mut output_bindings = Vec::new();
                let mut output_clients = Vec::new();
                // create one client per input party
                for (party, inputs) in &self.party_inputs {
                    let client = nodes.build_client().await;
                    let values_id =
                        Self::store_values(nodes, &self.program_id, invoker_client.user_id(), inputs.clone()).await;
                    value_ids.push(values_id);
                    input_bindings.push((party.clone(), client.user_id()));
                }
                // create one client per output party
                for party in self.party_outputs.keys() {
                    let client = nodes.build_client().await;
                    output_bindings.push((party.clone(), client.user_id()));
                    output_clients.push(client);
                }
                ExecutionSpec {
                    value_ids,
                    input_bindings,
                    output_bindings,
                    output_clients,
                    compute_inputs: Default::default(),
                }
            }
        }
    }

    async fn store_values(
        nodes: &Nodes,
        program_id: &str,
        invoker_user: UserId,
        inputs: HashMap<String, NadaValue<Clear>>,
    ) -> Uuid {
        let client = nodes.build_client().await;
        client
            .store_values()
            .add_values(inputs)
            .allow_compute(invoker_user, program_id.to_string())
            .ttl_days(1)
            .build()
            .expect("failed to build store values operaiton")
            .invoke()
            .await
            .expect("failed to store values")
    }

    async fn fetch_outputs(client: &VmClient, compute_id: Uuid) -> HashMap<String, NadaValue<Clear>> {
        client
            .retrieve_compute_results()
            .compute_id(compute_id)
            .build()
            .expect("failed to build invoke compute")
            .invoke()
            .await
            .expect("failed to fetch compute result")
            .expect("compute failed")
    }
}

struct ExecutionSpec {
    value_ids: Vec<Uuid>,
    input_bindings: Vec<(String, UserId)>,
    output_bindings: Vec<(String, UserId)>,
    output_clients: Vec<VmClient>,
    compute_inputs: HashMap<String, NadaValue<Clear>>,
}

/// The mode to use for clients.
#[derive(Default)]
pub enum ClientsMode {
    /// Use a single client for everything.
    Single,

    /// Use a separate client per party.
    #[default]
    OnePerParty,
}
