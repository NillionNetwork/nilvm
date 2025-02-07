//! Compute messages.

/// The protobuf model definitions.
pub mod proto {
    pub use crate::proto::compute::v1::*;
}

/// Rust types that can be converted from/to their protobuf counterparts.
#[cfg(feature = "rust-types")]
pub mod rust {
    use crate::{
        auth::rust::UserId, payments::rust::SignedReceipt, values::rust::NamedValue, ConvertProto, ProtoError,
        TransparentProto, TryIntoRust,
    };

    /// A response to a request to invoke a computation.
    pub type InvokeComputeResponse = super::proto::invoke::InvokeComputeResponse;

    /// A message for a compute stream.
    pub type ComputeStreamMessage = super::proto::stream::ComputeStreamMessage;

    /// A request to retrieve the results of a computation.
    pub type RetrieveResultsRequest = super::proto::retrieve::RetrieveResultsRequest;

    /// The result of a computation.
    pub type ComputationResult = super::proto::retrieve::ComputationResult;

    impl TransparentProto for InvokeComputeResponse {}
    impl TransparentProto for ComputeStreamMessage {}
    impl TransparentProto for RetrieveResultsRequest {}
    impl TransparentProto for ComputationResult {}

    /// A request to invoke a computation.
    #[derive(Clone, Debug, PartialEq)]
    pub struct InvokeComputeRequest {
        /// The receipt that proves this operation was paid for.
        pub signed_receipt: SignedReceipt,

        /// The ids of previously stored values being used as input parameters.
        pub value_ids: Vec<Vec<u8>>,

        /// The compute-time parameters.
        pub values: Vec<NamedValue>,

        /// The compute-time parameters in bincode format.
        pub bincode_values: Vec<u8>,

        /// The input party bindings.
        pub input_bindings: Vec<InputPartyBinding>,

        /// The output party bindings.
        pub output_bindings: Vec<OutputPartyBinding>,
    }

    impl ConvertProto for InvokeComputeRequest {
        type ProtoType = super::proto::invoke::InvokeComputeRequest;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                signed_receipt: Some(self.signed_receipt.into_proto()),
                value_ids: self.value_ids,
                values: self.values,
                bincode_values: self.bincode_values,
                input_bindings: self.input_bindings.into_iter().map(InputPartyBinding::into_proto).collect(),
                output_bindings: self.output_bindings.into_iter().map(OutputPartyBinding::into_proto).collect(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            let signed_receipt = model.signed_receipt.ok_or(ProtoError("'signed_receipt' not set"))?.try_into_rust()?;
            Ok(Self {
                signed_receipt,
                value_ids: model.value_ids,
                values: model.values,
                bincode_values: model.bincode_values,
                input_bindings: model
                    .input_bindings
                    .into_iter()
                    .map(InputPartyBinding::try_from_proto)
                    .collect::<Result<_, _>>()?,
                output_bindings: model
                    .output_bindings
                    .into_iter()
                    .map(OutputPartyBinding::try_from_proto)
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    // The response to a request to retrieve the results of a computation.
    pub enum RetrieveResultsResponse {
        WaitingComputation,
        Success { values: Vec<NamedValue>, bincode_values: Vec<u8> },
        Error { error: String },
    }

    impl ConvertProto for RetrieveResultsResponse {
        type ProtoType = super::proto::retrieve::RetrieveResultsResponse;

        fn into_proto(self) -> Self::ProtoType {
            use super::proto::retrieve::retrieve_results_response::State;
            match self {
                Self::WaitingComputation => Self::ProtoType { state: Some(State::WaitingComputation(())) },
                Self::Success { values, bincode_values } => {
                    Self::ProtoType { state: Some(State::Success(ComputationResult { values, bincode_values })) }
                }
                Self::Error { error } => Self::ProtoType { state: Some(State::Error(error)) },
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, crate::ProtoError> {
            use super::proto::retrieve::retrieve_results_response::State;
            let state = model.state.ok_or(ProtoError("'state' not set"))?;
            match state {
                State::WaitingComputation(()) => Ok(Self::WaitingComputation),
                State::Success(r) => Ok(Self::Success { values: r.values, bincode_values: r.bincode_values }),
                State::Error(error) => Ok(Self::Error { error }),
            }
        }
    }

    /// The bindings for input parties in a program.
    #[derive(Clone, Debug, PartialEq)]
    pub struct InputPartyBinding {
        /// The name of the party as defined in the program.
        pub party_name: String,

        /// The user identity this party is being bound to.
        pub user: UserId,
    }

    impl ConvertProto for InputPartyBinding {
        type ProtoType = super::proto::invoke::InputPartyBinding;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType { party_name: self.party_name, user: Some(self.user.into_proto()) }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            let user = model.user.ok_or(ProtoError("'user' not set"))?.try_into_rust()?;
            Ok(Self { party_name: model.party_name, user })
        }
    }

    /// The bindings for output parties in a program.
    #[derive(Clone, Debug, PartialEq)]
    pub struct OutputPartyBinding {
        /// The name of the party as defined in the program.
        pub party_name: String,

        /// The user identities this party is being bound to.
        pub users: Vec<UserId>,
    }

    impl ConvertProto for OutputPartyBinding {
        type ProtoType = super::proto::invoke::OutputPartyBinding;

        fn into_proto(self) -> Self::ProtoType {
            Self::ProtoType {
                party_name: self.party_name,
                users: self.users.into_iter().map(UserId::into_proto).collect(),
            }
        }

        fn try_from_proto(model: Self::ProtoType) -> Result<Self, ProtoError> {
            Ok(Self {
                party_name: model.party_name,
                users: model.users.into_iter().map(UserId::try_from_proto).collect::<Result<_, _>>()?,
            })
        }
    }
}
