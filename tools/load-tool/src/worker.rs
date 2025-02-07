//! Flow worker.

use crate::flow::{ErrorPolicy, ExecutionContext, Flow};
use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};

/// A worker that runs a flow in a loop.
pub struct Worker {
    context: ExecutionContext,
    policy: ErrorPolicy,
    running: &'static AtomicBool,
}

impl Worker {
    /// Construct a new worker.
    pub fn new(context: ExecutionContext, policy: ErrorPolicy, running: &'static AtomicBool) -> Self {
        Self { context, policy, running }
    }

    /// Run the given flow on this worker.
    pub async fn run(mut self, flow: Flow) -> anyhow::Result<()> {
        while self.running.load(Ordering::Acquire) {
            let result = flow.run(&mut self.context).await;
            if let Err(e) = &result {
                if self.policy.should_stop(e) {
                    warn!("Stopping flow because we received an error: {e:?}");
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
