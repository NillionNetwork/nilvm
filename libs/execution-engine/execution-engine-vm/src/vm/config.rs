//! Execution engine configuration properties

use crate::vm::plan::PlanStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Execution engine configuration properties
pub struct ExecutionVmConfig {
    /// Strategy that will be used to create the execution plan.
    pub plan_strategy: PlanStrategy,
    /// Max number of protocol messages by communication round
    pub max_protocol_messages_count: usize,
}

impl Default for ExecutionVmConfig {
    fn default() -> Self {
        ExecutionVmConfig { plan_strategy: PlanStrategy::Parallel, max_protocol_messages_count: 1000 }
    }
}
