pub(crate) mod interceptors;
pub(crate) mod metrics;

/// Resource quotas that can be exhausted via the API.
pub(crate) mod quotas {
    use node_api::errors::QuotaViolation;
    use once_cell::sync::Lazy;

    pub(crate) static PREPROCESSING: Lazy<QuotaViolation> =
        Lazy::new(|| QuotaViolation::new("PREPROCESSING", "preprocessing pool is exhausted"));
    pub(crate) static REQUESTS: Lazy<QuotaViolation> =
        Lazy::new(|| QuotaViolation::new("REQUESTS", "too many requests"));
}
