//! Uuid generation.

use uuid::Uuid;

/// A service to generate UUIDs.
#[cfg_attr(test, mockall::automock)]
pub(crate) trait UuidService: Send + Sync + 'static {
    fn generate(&self) -> Uuid;
}

pub(crate) struct DefaultUuidService;

impl UuidService for DefaultUuidService {
    fn generate(&self) -> Uuid {
        Uuid::new_v4()
    }
}
