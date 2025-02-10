use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait TimeService: Send + Sync + 'static {
    fn current_time(&self) -> DateTime<Utc>;
}

pub(crate) struct DefaultTimeService;

#[async_trait]
impl TimeService for DefaultTimeService {
    fn current_time(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
