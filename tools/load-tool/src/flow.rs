//! The various supported test flow types.

use crate::{clients_pool::Clients, report::FlowMetadata};
use futures::{channel::mpsc::Sender, SinkExt};
use math_lib::{
    impl_boxed_from_encoded_safe_prime,
    modular::{EncodedModulo, SafePrime},
};
use mpc_vm::address_count;
use nada_value::{clear::Clear, encoders::blob_chunk_size, NadaValue};
use nillion_client::{
    grpc::membership::{Cluster, Prime},
    operation::{InitialState, PaidOperation, PaidVmOperation},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, marker::PhantomData, sync::Arc, time::Instant};
use tokio::time::Duration;
use tracing::{error, warn};
use uuid::Uuid;

/// Flow executed by load test
#[derive(Serialize, Deserialize, Debug)]
pub enum FlowKind {
    /// Store values flow.
    StoreValues,

    /// Compute flow.
    Compute,

    /// Retrieve value flow.
    RetrieveValue,

    /// Request flow quote.
    RequestQuote,

    /// Store a program.
    StoreProgram,
}

/// A test flow.
#[derive(Clone)]
pub enum Flow {
    /// A store value flow.
    StoreValues(StoreValueArgs),

    /// A retrieve value flow.
    RetrieveValue(RetrieveValueArgs),

    /// A store program flow.
    StoreProgram(StoreProgramArgs),

    /// A compute flow.
    Compute(ComputeArgs),
}

impl Flow {
    /// Run this flow.
    pub(crate) async fn run(&self, context: &mut ExecutionContext) -> Result<(), FlowError> {
        use Flow::*;
        match self {
            StoreValues(args) => Self::run_store_value(args, context).await,
            StoreProgram(args) => Self::run_store_program(args, context).await,
            Compute(args) => Self::run_compute(args, context).await,
            RetrieveValue(args) => Self::run_retrieve_value(args, context).await,
        }
    }

    /// Get the metadata for this flow.
    pub(crate) async fn metadata(&self, cluster_info: Cluster) -> anyhow::Result<FlowMetadata> {
        use Flow::*;
        let (prime_size, modulo) = match cluster_info.prime {
            Prime::Safe64Bits => (8, EncodedModulo::U64SafePrime),
            Prime::Safe128Bits => (16, EncodedModulo::U64SafePrime),
            Prime::Safe256Bits => (32, EncodedModulo::U64SafePrime),
        };
        let (kind, secrets) = match self {
            StoreValues(args) => (FlowKind::StoreValues, args.values.as_ref().clone()),
            Compute(args) => (FlowKind::Compute, args.values.clone()),
            RetrieveValue(args) => (FlowKind::RetrieveValue, HashMap::from([("".to_string(), args.value.clone())])),
            StoreProgram(_) => (FlowKind::StoreProgram, Default::default()),
        };
        let chunk_count = Box::<dyn ChunkCount>::try_from(&modulo)?.chunk_count(&secrets);
        let secrets_size = prime_size * chunk_count;
        Ok(FlowMetadata { kind, secret_count: secrets.len() as u32, secrets_size })
    }

    async fn run_operation<O>(
        operation: PaidOperation<'_, O, InitialState>,
    ) -> Result<(O::Output, OperationSummary), FlowError>
    where
        O: PaidVmOperation,
    {
        let start = Instant::now();
        let operation = operation.quote().await.map_err(|e| {
            error!("Failed to get quote: {e}");
            FlowError::QuoteError
        })?;
        let quote_duration = start.elapsed();
        let before_payment = Instant::now();
        let operation = operation.pay().await.map_err(|e| {
            error!("Failed to make payment: {e}");
            FlowError::PaymentError
        })?;
        let payment_duration = before_payment.elapsed();
        let operation = operation.validate().await.map_err(|e| {
            error!("Failed to get receipt: {e}");
            FlowError::ReceiptError
        })?;

        let operation_start = Instant::now();
        let output = operation.invoke().await.map_err(|e| {
            error!("Failed to run operation: {e}");
            if e.to_string().contains("not enough ") {
                FlowError::PreprocessingExhausted
            } else {
                FlowError::OperationFailed
            }
        })?;
        let operation_duration = operation_start.elapsed();

        let summary = OperationSummary { quote_duration, payment_duration, operation_duration };
        Ok((output, summary))
    }

    async fn run_store_value(args: &StoreValueArgs, context: &mut ExecutionContext) -> Result<(), FlowError> {
        let ExecutionContext { clients, sender } = context;
        let values = args.values.as_ref().clone();

        let start = Instant::now();
        let operation = clients.vm.store_values().ttl_days(1).add_values(values.clone()).build().expect("build failed");
        let result = Self::run_operation(operation).await;
        let result = result.map(|r| r.1);
        Self::send_report(start, result, sender).await
    }

    async fn run_compute(args: &ComputeArgs, context: &mut ExecutionContext) -> Result<(), FlowError> {
        let ExecutionContext { clients, sender } = context;
        let ComputeArgs { program_id, input_parties, output_parties, values: variables, store_ids } = args;

        let start = Instant::now();
        let client = &clients.vm;
        let mut builder = client
            .invoke_compute()
            .program_id(program_id.clone())
            .add_values(variables.clone())
            .add_value_ids(store_ids.clone());
        for name in input_parties {
            builder = builder.bind_input_party(name.clone(), client.user_id());
        }
        for name in output_parties {
            builder = builder.bind_output_party(name.clone(), vec![client.user_id()]);
        }
        let operation = builder.build().expect("build failed");
        let result = Self::run_operation(operation).await;
        let result = match result {
            Ok((compute_id, summary)) => client
                .retrieve_compute_results()
                .compute_id(compute_id)
                .build()
                .expect("build failed")
                .invoke()
                .await
                .map_err(|e| {
                    error!("Fetching compute results failed: {e}");
                    FlowError::OperationFailed
                })
                .map(|_| summary),
            Err(e) => Err(e),
        };
        Self::send_report(start, result, sender).await
    }

    async fn run_retrieve_value(args: &RetrieveValueArgs, context: &mut ExecutionContext) -> Result<(), FlowError> {
        let ExecutionContext { clients, sender } = context;
        let values_id = args.values_id;
        let start = Instant::now();
        let operation = clients.vm.retrieve_values().values_id(values_id).build().expect("build failed");
        let result = Self::run_operation(operation).await.map(|r| r.1);
        Self::send_report(start, result, sender).await
    }

    async fn run_store_program(args: &StoreProgramArgs, context: &mut ExecutionContext) -> Result<(), FlowError> {
        let ExecutionContext { clients, sender } = context;
        let name: String = rand::random::<[char; 32]>().iter().collect();
        let start = Instant::now();
        let operation =
            clients.vm.store_program().name(name).program(args.raw_program.clone()).build().expect("build failed");
        let result = Self::run_operation(operation).await.map(|r| r.1);
        Self::send_report(start, result, sender).await
    }

    async fn send_report(
        start_time: Instant,
        result: Result<OperationSummary, FlowError>,
        sender: &mut Sender<FlowSummary>,
    ) -> Result<(), FlowError> {
        let elapsed = start_time.elapsed();
        let status = match &result {
            Ok(_) => FlowStatus::Success,
            Err(e) => {
                warn!("Flow failed: {e:?}");
                FlowStatus::Failure
            }
        };
        let OperationSummary { quote_duration, payment_duration, operation_duration } =
            result.clone().unwrap_or_default();
        let summary = FlowSummary { elapsed, quote_duration, payment_duration, operation_duration, status };
        sender.send(summary).await.map_err(|_| FlowError::RunnerStopped)?;
        result.map(|_| ())
    }
}

#[derive(Default, Clone)]
struct OperationSummary {
    quote_duration: Duration,
    payment_duration: Duration,
    operation_duration: Duration,
}

/// A store value test flow's arguments.
#[derive(Clone)]
pub struct StoreValueArgs {
    /// The operation's values.
    pub values: Arc<HashMap<String, NadaValue<Clear>>>,
}

/// A compute flow's arguments.
#[derive(Clone)]
pub struct ComputeArgs {
    /// The program id.
    pub program_id: String,

    /// The operation's variables.
    pub values: HashMap<String, NadaValue<Clear>>,

    /// The program input parties.
    pub input_parties: Vec<String>,

    /// The program output parties.
    pub output_parties: Vec<String>,

    /// The referenced store ids.
    pub store_ids: Vec<Uuid>,
}

/// A retrieve value flow's arguments.
#[derive(Clone)]
pub struct RetrieveValueArgs {
    /// The values id to retrieve.
    pub values_id: Uuid,

    /// The value being retrieved.
    pub value: NadaValue<Clear>,
}

/// A store program flow's arguments.
#[derive(Clone)]
pub struct StoreProgramArgs {
    /// The raw program to be uploaded.
    pub raw_program: Vec<u8>,
}

/// The execution context for a flow.
#[derive(Clone)]
pub struct ExecutionContext {
    /// The nillion client to be used.
    pub clients: Clients,

    /// The channel in which results should be sent.
    pub sender: Sender<FlowSummary>,
}

/// A flow's result summary.
pub struct FlowSummary {
    /// The amount of time this flow took.
    pub elapsed: Duration,

    /// The amount of time quote took
    pub quote_duration: Duration,

    /// The amount of time payment took
    pub payment_duration: Duration,

    /// The amount of time operation took
    pub operation_duration: Duration,

    /// The result.
    pub status: FlowStatus,
}

/// A flow's result.
#[derive(Debug)]
pub enum FlowStatus {
    /// A successful run.
    Success,

    /// A failed run.
    Failure,
}

/// The policy used when an error is encountered.
#[derive(Clone, Debug, Deserialize)]
pub enum ErrorPolicy {
    /// Always stop the test on error.
    AlwaysStop,

    /// Stop the test if the network ran out of preprocessing elements.
    StopOnPreprocessingExhausted,

    /// Ignore all errors.
    Ignore,
}

impl ErrorPolicy {
    /// Check whether the given error should cause us to stop based on this policy.
    pub(crate) fn should_stop(&self, error: &FlowError) -> bool {
        match (self, error) {
            (Self::AlwaysStop, _) => true,
            (Self::StopOnPreprocessingExhausted, FlowError::PreprocessingExhausted) => true,
            (Self::StopOnPreprocessingExhausted, _) => false,
            (Self::Ignore, _) => false,
        }
    }
}

/// An error during the execution of a flow.
#[derive(Debug, Clone)]
pub(crate) enum FlowError {
    /// A preprocessing pool was exhausted.
    PreprocessingExhausted,

    // We failed to get a quote.
    QuoteError,

    // We failed to get a receipt.
    ReceiptError,

    /// The operation failed.
    OperationFailed,

    /// The runner stopped and dropped our channel.
    RunnerStopped,

    /// An error occurred during a payment.
    PaymentError,
}

#[derive(Default)]
struct DefaultChunkCount<T>(PhantomData<T>);

/// Return the number of chunk elements for the type.
///
/// - For primitive types, it returns 1
/// - For Blobs it returns BLOB_SIZE / BLOB_CHUNK_SIZE
/// - For container types it returns the total amount of contained elements plus one (required to store the header)
fn chunk_element_count<T>(nada_value: &NadaValue<Clear>) -> u32
where
    T: SafePrime,
{
    if let NadaValue::SecretBlob(value) = nada_value {
        let chunk_size = blob_chunk_size::<T>();
        value.len().div_ceil(chunk_size) as u32
    } else {
        address_count(&nada_value.to_type()).unwrap_or(1) as u32
    }
}

trait ChunkCount {
    fn chunk_count(&self, secrets: &HashMap<String, NadaValue<Clear>>) -> u32;
}

impl<T: SafePrime> ChunkCount for DefaultChunkCount<T> {
    fn chunk_count(&self, secrets: &HashMap<String, NadaValue<Clear>>) -> u32 {
        secrets.values().map(|s| chunk_element_count::<T>(s)).sum()
    }
}

impl_boxed_from_encoded_safe_prime!(DefaultChunkCount, ChunkCount);

#[cfg(test)]
mod tests {
    use crate::flow::chunk_element_count;
    use math_lib::modular::U64SafePrime;
    use nada_value::{clear::Clear, NadaType, NadaValue};
    use rstest::rstest;
    type Prime = U64SafePrime;

    fn test_compound_array() -> NadaValue<Clear> {
        let array1 = NadaValue::new_array(
            NadaType::SecretInteger,
            [1, 2, 3, 4, 5].map(|i| NadaValue::new_secret_integer(i)).to_vec(),
        )
        .unwrap();
        let array2 = NadaValue::new_array(
            NadaType::SecretInteger,
            [5, 4, 3, 2, 5].map(|i| NadaValue::new_secret_integer(i)).to_vec(),
        )
        .unwrap();
        NadaValue::new_array(array1.to_type(), vec![array1, array2]).unwrap()
    }

    fn test_compound_tuple() -> NadaValue<Clear> {
        let array1 = NadaValue::new_array(
            NadaType::SecretInteger,
            [1, 2, 3, 4, 5].map(|i| NadaValue::new_secret_integer(i)).to_vec(),
        )
        .unwrap();
        let array2 = NadaValue::new_array(
            NadaType::SecretInteger,
            [5, 4, 3, 2, 5].map(|i| NadaValue::new_secret_integer(i)).to_vec(),
        )
        .unwrap();
        NadaValue::new_tuple(array1, array2).unwrap()
    }

    #[rstest]
    #[case(NadaValue::new_secret_integer(-23),1)] // Primitive elements are stored in 1 chunk
    #[case(test_compound_array(), 13)] // (5 + 1) * 2 + 1, elements + container type headers
    #[case(test_compound_tuple(), 13)] // (5 + 1) * 2 + 1
    #[case(NadaValue::new_secret_blob(vec![1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,7,8,9,0]),8)] // For 64-bit chunk size is 7 ( 64/8 - 1 ), so ceil(50/7) = 8
    fn test_chunk_element_count(#[case] value: NadaValue<Clear>, #[case] expected: u32) {
        assert_eq!(chunk_element_count::<Prime>(&value), expected)
    }
}
