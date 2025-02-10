use governor::{
    clock::{Clock, DefaultClock},
    DefaultKeyedRateLimiter, Quota, RateLimiter,
};
use grpc_channel::auth::AuthenticateRequest;
use node_api::{
    auth::rust::UserId,
    errors::{ErrorDetails, StatusExt},
};
use std::{collections::HashSet, net::IpAddr, sync::Arc};
use tonic::{service::Interceptor, Code, Request, Status};

/// An interceptor that restricts access to an internal API.
#[derive(Clone)]
pub(crate) struct InternalServiceInterceptor {
    allowed_users: Arc<HashSet<UserId>>,
}

impl InternalServiceInterceptor {
    pub(crate) fn new<I: IntoIterator<Item = UserId>>(allowed_users: I) -> Self {
        let allowed_users = Arc::new(allowed_users.into_iter().collect());
        Self { allowed_users }
    }
}

impl Interceptor for InternalServiceInterceptor {
    fn call(&mut self, request: Request<()>) -> tonic::Result<Request<()>> {
        let user_id = request.user_id()?;
        if self.allowed_users.contains(&user_id) {
            Ok(request)
        } else {
            Err(Status::permission_denied("user does not have permissions to invoke service"))
        }
    }
}

#[derive(Clone)]
pub(crate) struct RateLimitInterceptor {
    ignored_users: Arc<HashSet<UserId>>,
    limiter: Arc<DefaultKeyedRateLimiter<IpAddr>>,
}

impl RateLimitInterceptor {
    pub(crate) fn new<I: IntoIterator<Item = UserId>>(ignored_users: I, quota: Quota) -> Self {
        let ignored_users = Arc::new(ignored_users.into_iter().collect());
        let limiter = RateLimiter::keyed(quota);
        Self { ignored_users, limiter: limiter.into() }
    }
}

impl Interceptor for RateLimitInterceptor {
    fn call(&mut self, request: Request<()>) -> tonic::Result<Request<()>> {
        // If the user is authenticated is in the ignore list, let it through directly
        if let Ok(user_id) = request.user_id() {
            if self.ignored_users.contains(&user_id) {
                return Ok(request);
            }
        }
        let peer_ip =
            request.remote_addr().map(|addr| addr.ip()).ok_or_else(|| Status::internal("no peer IP found"))?;
        match self.limiter.check_key(&peer_ip) {
            Ok(_) => Ok(request),
            Err(e) => {
                let wait_time = e.wait_time_from(DefaultClock::default().now());
                let mut details = ErrorDetails::new();
                details.set_retry_info(Some(wait_time)).set_quota_failure(vec![crate::grpc::quotas::REQUESTS.clone()]);
                Err(Status::with_error_details(
                    Code::ResourceExhausted,
                    format!("too many requests, try again in {wait_time:?}"),
                    details,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controllers::tests::MakeAuthenticated;

    #[test]
    fn allow_users() {
        let user = UserId::from_bytes("bob");
        let mut interceptor = InternalServiceInterceptor::new([user.clone()]);
        let request = Request::new(()).authenticated(user.clone());
        interceptor.call(request).expect("allowed user is not allowed");

        let request = Request::new(()).authenticated(UserId::from_bytes("mike"));
        interceptor.call(request).expect_err("disallowed user is allowed");
    }
}
