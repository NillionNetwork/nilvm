use log::warn;
use metrics::{
    maybe::MaybeMetric,
    metrics::SingleHistogramMetric,
    prelude::{BuildTimer, CounterMetric, HistogramMetric, ScopedTimer, SingleCounterMetric, TimingBuckets},
    Counter, Histogram,
};
use once_cell::sync::Lazy;
use reqwest::Client as HttpClient;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::{collections::HashMap, time::Duration};
use tokio::sync::Mutex;
use tracing::info;

const SIMPLE_PRICE_URL: &str = "https://pro-api.coingecko.com/api/v3/simple/price";

static METRICS: Lazy<Metrics> = Lazy::new(Metrics::default);

/// A TokenDollarConversion error.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub(crate) enum TokenDollarConversionError {
    /// An internal error.
    #[error("internal: {0}")]
    Internal(String),
}

/// Token Dollar Conversion Service.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait TokenDollarConversionService: Send + Sync + 'static {
    /// Get token price in dollars.
    async fn token_dollar_price(&self) -> Result<Decimal, TokenDollarConversionError>;
}

/// Token Dollar Conversion CoinGecko service.
pub struct TokenDollarConversionCoinGeckoService {
    http_client: HttpClient,
    coingecko_api_key: String,
    coin_id: String,
    simple_price_url: &'static str,
    last_check_and_value: Mutex<(tokio::time::Instant, Decimal)>,
}

impl TokenDollarConversionCoinGeckoService {
    pub fn new(coingecko_api_key: String, coin_id: String) -> Self {
        Self {
            http_client: HttpClient::new(),
            coingecko_api_key,
            coin_id,
            simple_price_url: SIMPLE_PRICE_URL,
            #[allow(clippy::arithmetic_side_effects)]
            last_check_and_value: Mutex::new((tokio::time::Instant::now() - Duration::from_secs(61), Decimal::from(0))),
        }
    }
}

#[async_trait::async_trait]
impl TokenDollarConversionService for TokenDollarConversionCoinGeckoService {
    async fn token_dollar_price(&self) -> Result<Decimal, TokenDollarConversionError> {
        let now = tokio::time::Instant::now();
        let mut last_check_and_value = self.last_check_and_value.lock().await;

        #[allow(clippy::arithmetic_side_effects)]
        if now - last_check_and_value.0 < Duration::from_secs(60) {
            return Ok(last_check_and_value.1);
        }

        let params = [("ids", self.coin_id.as_str()), ("vs_currencies", "usd")];
        info!("Fetching token price from CoinGecko");
        let timer = METRICS.price_query_timer();

        let response = self
            .http_client
            .get(self.simple_price_url)
            .query(&params)
            .header("X-CG-PRO-API-KEY", &self.coingecko_api_key)
            .send()
            .await;

        drop(timer);

        let response = match response {
            Ok(response) => response,
            Err(e) => {
                warn!("Failed to fetch token price from CoinGecko: {e}");
                METRICS.inc_query_errors(&e.to_string());
                return Err(TokenDollarConversionError::Internal(e.to_string()));
            }
        };

        let response: HashMap<String, Price> =
            response.json().await.map_err(|e| TokenDollarConversionError::Internal(e.to_string()))?;

        let price = response.get(&self.coin_id).map(|response| response.usd).ok_or_else(|| {
            TokenDollarConversionError::Internal("CoinGecko response does not contain the requested coin".to_string())
        })?;
        // Just in case...
        if price <= Decimal::from(0) {
            return Err(TokenDollarConversionError::Internal(format!("token price is <= 0 ({price})")));
        }

        info!("Token price from CoinGecko: {price}");

        *last_check_and_value = (now, price);
        Ok(price)
    }
}

/// A conversion service that uses a hardcoded price.
///
/// This is only used in devnets and testing networks.
pub struct HardcodedTokenDollarConversionService {
    price: Decimal,
}

impl HardcodedTokenDollarConversionService {
    pub fn new(price: Decimal) -> Self {
        Self { price }
    }
}

#[async_trait::async_trait]
impl TokenDollarConversionService for HardcodedTokenDollarConversionService {
    async fn token_dollar_price(&self) -> Result<Decimal, TokenDollarConversionError> {
        Ok(self.price)
    }
}

/// Price from CoinGecko Simple Price API
#[derive(Debug, Deserialize)]
struct Price {
    usd: Decimal,
}

struct Metrics {
    price_query_duration: MaybeMetric<Histogram<Duration>>,
    price_query_errors: MaybeMetric<Counter>,
}

impl Default for Metrics {
    fn default() -> Self {
        let price_query_duration = Histogram::new(
            "token_price_duration_seconds",
            "Duration of token price query to coingecko",
            &[],
            TimingBuckets::sub_second(),
        )
        .into();
        let price_query_errors = Counter::new(
            "token_price_errors_total",
            "Number of errors encountered in token price query to coingecko",
            &["code"],
        )
        .into();
        Self { price_query_duration, price_query_errors }
    }
}

impl Metrics {
    fn price_query_timer(&self) -> ScopedTimer<impl SingleHistogramMetric<Duration>> {
        self.price_query_duration.with_labels([]).into_timer()
    }

    fn inc_query_errors(&self, error: &str) {
        self.price_query_errors.with_labels([("error", error)]).inc();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_with::env(COINGECKO_API_KEY)]
    #[tokio::test]
    async fn test_get_token_dollar_price() {
        let coingecko_api_key = std::env::var("COINGECKO_API_KEY").unwrap();
        let coin_id = "cosmos".to_string();
        let service = TokenDollarConversionCoinGeckoService {
            http_client: HttpClient::new(),
            coingecko_api_key,
            coin_id,
            simple_price_url: DEMO_SIMPLE_PRICE_URL,
            last_check_and_value: Mutex::new((tokio::time::Instant::now() - Duration::from_secs(61), Decimal::from(0))),
        };
        let price = service.token_dollar_price().await.unwrap();

        assert!(price > Decimal::from(0));
    }

    const DEMO_SIMPLE_PRICE_URL: &str = "https://api.coingecko.com/api/v3/simple/price";
}
