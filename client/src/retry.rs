//! Request retry utilities.

use futures::{future, FutureExt};
use nillion_client_core::values::PartyId;
use node_api::errors::StatusExt;
use std::{fmt, future::Future, iter, mem, time::Duration};
use tonic::{async_trait, Code, Status};
use tracing::{info, warn};

const DEFAULT_MAX_RETRIES: usize = 10;
pub(crate) const RETRY_CODES: &[Code] =
    &[Code::DeadlineExceeded, Code::ResourceExhausted, Code::Unavailable, Code::Unknown];
const RETRY_DELAYS: &[Duration] = &[Duration::from_secs(1), Duration::from_secs(3), Duration::from_secs(5)];

struct PartyRequest<'a, P, C, R> {
    party: P,
    client: &'a C,
    request: R,
}

/// Allows retrying client requests.
///
/// This will retry each failed client request until all nodes reply with a success. If the max
/// retries are reached, the last failure will be returned for nodes that failed.
pub(crate) struct Retrier<'a, C, R, P = PartyId, S = TokioSleeper> {
    requests: Vec<PartyRequest<'a, P, C, R>>,
    max_retries: usize,
    sleeper: S,
}

impl<'a, C, R, P> Default for Retrier<'a, C, R, P, TokioSleeper> {
    fn default() -> Self {
        Self { requests: Default::default(), max_retries: DEFAULT_MAX_RETRIES, sleeper: TokioSleeper }
    }
}

impl<'a, C, R, P, S> Retrier<'a, C, R, P, S>
where
    R: Clone,
    P: fmt::Display,
    S: Sleeper,
{
    pub(crate) fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub(crate) fn retry_delays() -> impl Iterator<Item = &'static Duration> {
        #[allow(clippy::unwrap_used)]
        // SAFETY: this is a non empty slice so `last` can't fail
        // use all retry delays, then repeat the last one forever
        RETRY_DELAYS.iter().chain(iter::repeat(RETRY_DELAYS.last().unwrap()))
    }

    pub(crate) fn add_request(&mut self, party: P, client: &'a C, request: R) {
        let request = PartyRequest { party, client, request };
        self.requests.push(request);
    }

    pub(crate) async fn invoke_mapped<I, F, O>(self, invoke_request: I) -> Vec<(P, tonic::Result<O>)>
    where
        I: Fn(&'a C, R) -> F,
        F: Future<Output = tonic::Result<O>>,
    {
        let Self { requests, max_retries, sleeper } = self;
        let mut finished = Vec::new();
        let mut pending = requests;
        let mut delays = Self::retry_delays();
        let mut retries = 0;
        while !pending.is_empty() {
            let mut requested_retry_delay = None;
            let mut futs = Vec::new();
            for request in mem::take(&mut pending) {
                let fut = invoke_request(request.client, request.request.clone());
                let fut = fut.map(|r| (request, r));
                futs.push(fut);
            }
            let results = future::join_all(futs).await;
            for (request, result) in results {
                match result {
                    Err(e) if RETRY_CODES.contains(&e.code()) && retries < max_retries => {
                        warn!("Request failed for {}, retrying it", request.party);
                        // If the node suggested a delay, use the max delay we've been suggested
                        if let Some(info) = e.get_error_details().retry_info() {
                            requested_retry_delay = requested_retry_delay.max(info.retry_delay);
                        }
                        pending.push(request);
                    }
                    // store the result if:
                    // * it's successful
                    // * it's a non retryable error
                    // * we hit the max attempts
                    Ok(_) | Err(_) => finished.push((request.party, result)),
                };
            }
            if !pending.is_empty() {
                let delay = match requested_retry_delay {
                    Some(delay) => {
                        info!("Using server suggested retry delay {delay:?}");
                        delay
                    }
                    None => {
                        // SAFETY: `delays` is an infinite iterator
                        #[allow(clippy::expect_used)]
                        *delays.next().expect("no more delays")
                    }
                };
                retries = retries.saturating_add(1);

                let total_pending = pending.len();
                info!(
                    "Need to retry {total_pending} requests, sleeping for {delay:?} ({retries} / {max_retries} retries)"
                );
                sleeper.sleep(delay).await;
            }
        }
        finished
    }

    pub(crate) async fn invoke<I, F, O>(self, invoke_request: I) -> Vec<tonic::Result<O>>
    where
        I: Fn(&'a C, R) -> F,
        F: Future<Output = tonic::Result<O>>,
    {
        self.invoke_mapped(invoke_request).await.into_iter().map(|(_party, output)| output).collect()
    }

    pub(crate) async fn invoke_single<I, F, O>(self, invoke_request: I) -> tonic::Result<O>
    where
        I: Fn(&'a C, R) -> F,
        F: Future<Output = tonic::Result<O>>,
    {
        let results = self.invoke(invoke_request).await;
        results.into_iter().next().ok_or_else(|| Status::failed_precondition("expected one result"))?
    }
}

#[async_trait]
pub(crate) trait Sleeper {
    async fn sleep(&self, duration: Duration);
}

pub(crate) struct TokioSleeper;

#[async_trait]
impl Sleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{HashMap, VecDeque},
        sync::Mutex,
    };
    use tonic::Status;

    struct DummySleeper;

    #[async_trait]
    impl Sleeper for DummySleeper {
        async fn sleep(&self, _duration: Duration) {}
    }

    #[derive(Default)]
    struct Client {
        errors: Mutex<VecDeque<Status>>,
    }

    impl Client {
        fn new(errors: &[Status]) -> Self {
            let errors = errors.iter().cloned().collect();
            Self { errors: Mutex::new(errors) }
        }
    }

    impl Client {
        async fn handle(&self, input: i32) -> tonic::Result<i32> {
            let mut errors = self.errors.lock().unwrap();
            if let Some(error) = errors.pop_front() { Err(error) } else { Ok(input) }
        }
    }

    fn make_retrier<'a>(max_retries: usize) -> Retrier<'a, Client, i32, PartyId, DummySleeper> {
        Retrier { sleeper: DummySleeper, requests: Vec::new(), max_retries }
    }

    #[tokio::test]
    async fn single_success() {
        // 0 retries allowed
        let mut retrier = make_retrier(0);
        let client = Client::default();
        retrier.add_request(PartyId::from(vec![1]), &client, 1);

        let result = retrier.invoke(Client::handle).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_ref().unwrap(), &1);
    }

    #[tokio::test]
    async fn single_retry() {
        // 1 retries allowed
        let mut retrier = make_retrier(1);
        let client = Client::new(&[Status::deadline_exceeded("timeout")]);
        retrier.add_request(PartyId::from(vec![1]), &client, 1);

        let result = retrier.invoke(Client::handle).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_ref().unwrap(), &1);
    }

    #[tokio::test]
    async fn single_retry_failure() {
        // 0 retries allowed
        let mut retrier = make_retrier(0);
        let client = Client::new(&[Status::deadline_exceeded("timeout")]);
        retrier.add_request(PartyId::from(vec![1]), &client, 1);

        let result = retrier.invoke(Client::handle).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_ref().unwrap_err().code(), Code::DeadlineExceeded);
    }

    #[tokio::test]
    async fn retry_one_failure_out_of_two() {
        // 1 retries allowed
        let mut retrier = make_retrier(1);
        let client1 = Client::new(&[Status::deadline_exceeded("timeout")]);
        let client2 = Client::new(&[]);
        retrier.add_request(PartyId::from(vec![1]), &client1, 1);
        retrier.add_request(PartyId::from(vec![2]), &client2, 1);

        let result = retrier.invoke(Client::handle).await;
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(Result::is_ok), "not all Ok: {result:?}");
    }

    #[tokio::test]
    async fn fail_after_retry() {
        // 1 retries allowed
        let mut retrier = make_retrier(1);
        let client1 = Client::new(&[Status::deadline_exceeded("timeout 1"), Status::deadline_exceeded("timeout 2")]);
        let client2 = Client::new(&[]);
        retrier.add_request(PartyId::from(vec![1]), &client1, 1);
        retrier.add_request(PartyId::from(vec![2]), &client2, 1);

        let results = retrier.invoke_mapped(Client::handle).await;
        let results: HashMap<_, _> = results.into_iter().collect();
        assert_eq!(
            results
                .get(&PartyId::from(vec![1]))
                .as_ref()
                .expect("party 1 not found")
                .as_ref()
                .expect_err("not an error")
                .message(),
            "timeout 2"
        );
        assert_eq!(
            results.get(&PartyId::from(vec![2])).as_ref().expect("party 2 not found").as_ref().expect("not Ok"),
            &1
        );
    }
}
