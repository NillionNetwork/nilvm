//! gRPC channel utilities.

#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::iterator_step_by_zero,
    clippy::invalid_regex,
    clippy::string_slice,
    clippy::unimplemented,
    clippy::todo
)]

use auth::ClientAuthInterceptor;
use prost::bytes::Bytes;
use std::time::Duration;
use token::TokenAuthenticator;
use tonic::{
    service::interceptor::InterceptedService,
    transport::{Body, Certificate, ClientTlsConfig},
};
use tower::timeout::Timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A boxed std::error::Error;
pub type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub mod auth;
pub mod token;

/// An unauthenticated channel tag.
pub struct Unauthenticated;

/// An authenticated channel tag.
pub struct Authenticated(TokenAuthenticator);

/// The configuration for a gRPC channel.
pub struct GrpcChannelConfig<T = Unauthenticated> {
    url: String,
    tls_config: ClientTlsConfig,
    use_native_roots: bool,
    authentication: T,
    timeout: Duration,
}

impl GrpcChannelConfig<Unauthenticated> {
    /// Construct a new channel configuration for the given URL.
    ///
    /// Note that channels are unauthenticated unless [GrpcChannelConfig::authentication] is
    /// called.
    pub fn new<S: Into<String>>(url: S) -> Self {
        Self {
            url: url.into(),
            tls_config: ClientTlsConfig::default(),
            use_native_roots: true,
            authentication: Unauthenticated,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl<T> GrpcChannelConfig<T> {
    /// Set the certificate for this connection.
    ///
    /// when this is not set and the URL uses the 'https' scheme, the sever's certificate is
    /// expected to be signed by a trusted root CA.
    pub fn ca_certificate(mut self, certificate: &[u8]) -> Self {
        self.tls_config = self.tls_config.ca_certificate(Certificate::from_pem(certificate));
        self.use_native_roots = false;
        self
    }

    /// Set the domain name to expect on the server's certificate.
    ///
    /// This is only necessary when the server's certificate doesn't match the domain we are
    /// connecting to.
    pub fn domain<S: Into<String>>(mut self, domain: S) -> Self {
        self.tls_config = self.tls_config.domain_name(domain);
        self
    }

    /// Enable authentication on this channel using the provided authenticator.
    pub fn authentication(self, authenticator: TokenAuthenticator) -> GrpcChannelConfig<Authenticated> {
        GrpcChannelConfig {
            url: self.url,
            tls_config: self.tls_config,
            use_native_roots: self.use_native_roots,
            authentication: Authenticated(authenticator),
            timeout: self.timeout,
        }
    }

    /// Set the channel's timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn build_channel(self) -> Result<tonic::transport::Channel, GrpcChannelError> {
        let endpoint = tonic::transport::Channel::from_shared(self.url)
            .map_err(|e| GrpcChannelError::InvalidUrl(e.to_string()))?;
        let mut tls_config = self.tls_config;
        if self.use_native_roots {
            tls_config = tls_config.with_native_roots();
        }
        let endpoint =
            endpoint.tls_config(tls_config).map_err(|e| GrpcChannelError::InvalidTlsConfig(e.to_string()))?;
        let channel = endpoint.connect_lazy();
        Ok(channel)
    }
}

impl GrpcChannelConfig<Unauthenticated> {
    /// Build an unauthenticated gRPC channel from this config.
    pub fn build(self) -> Result<UnauthenticatedGrpcChannel, GrpcChannelError> {
        let timeout = self.timeout;
        let channel = self.build_channel()?;
        Ok(UnauthenticatedGrpcChannel(Timeout::new(channel, timeout)))
    }
}

impl GrpcChannelConfig<Authenticated> {
    /// Build an authenticated gRPC channel from this config.
    pub fn build(self) -> Result<AuthenticatedGrpcChannel, GrpcChannelError> {
        let timeout = self.timeout;
        let interceptor = ClientAuthInterceptor::new(self.authentication.0.clone());
        let channel = self.build_channel()?;
        Ok(AuthenticatedGrpcChannel(Timeout::new(channel, timeout), interceptor))
    }
}

/// A gRPC channel error.
#[derive(Debug, thiserror::Error)]
pub enum GrpcChannelError {
    /// An invalid URL was provided.
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// The TLS config is invalid.
    #[error("invalid TLS config: {0}")]
    InvalidTlsConfig(String),
}

/// A gRPC channel which is not authenticated.
#[derive(Clone)]
pub struct UnauthenticatedGrpcChannel(Timeout<tonic::transport::Channel>);

impl UnauthenticatedGrpcChannel {
    /// Enable authentication on this channel.
    pub fn authenticated(self, authenticator: TokenAuthenticator) -> AuthenticatedGrpcChannel {
        AuthenticatedGrpcChannel(self.0, ClientAuthInterceptor::new(authenticator))
    }
}

/// A gRPC channel that is authenticated.
#[derive(Clone)]
pub struct AuthenticatedGrpcChannel(Timeout<tonic::transport::Channel>, ClientAuthInterceptor);

/// A channel that can be used as a transport for a gRPC service.
pub trait TransportChannel {
    /// The associated channel type for this channel.
    type Channel: tonic::client::GrpcService<
            tonic::body::BoxBody,
            ResponseBody: Body<Data = Bytes, Error: Into<StdError> + Send> + Send + 'static,
            Error: Into<StdError>,
        > + Clone
        + 'static;

    /// Get the channel type.
    fn into_channel(self) -> Self::Channel;

    /// Turn this into an unauthenticated channel
    fn into_unauthenticated(self) -> UnauthenticatedGrpcChannel;
}

impl TransportChannel for UnauthenticatedGrpcChannel {
    type Channel = Timeout<tonic::transport::Channel>;

    fn into_channel(self) -> Self::Channel {
        self.0
    }

    fn into_unauthenticated(self) -> UnauthenticatedGrpcChannel {
        self
    }
}

impl TransportChannel for AuthenticatedGrpcChannel {
    type Channel = InterceptedService<Timeout<tonic::transport::Channel>, ClientAuthInterceptor>;

    fn into_channel(self) -> Self::Channel {
        InterceptedService::new(self.0, self.1)
    }

    fn into_unauthenticated(self) -> UnauthenticatedGrpcChannel {
        UnauthenticatedGrpcChannel(self.0)
    }
}
