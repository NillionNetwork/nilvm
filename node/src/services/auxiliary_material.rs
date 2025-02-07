use super::blob::BlobService;
use crate::storage::{
    repositories::{
        auxiliary_material_meta::{AuxiliaryMaterialMetadata, AuxiliaryMaterialMetadataRepository},
        blob::BlobRepositoryError,
    },
    sqlite::DatabaseError,
};
use async_trait::async_trait;
use node_api::preprocessing::rust::AuxiliaryMaterial;
use protocols::threshold_ecdsa::auxiliary_information::output::EcdsaAuxInfo;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub(crate) type EcdaAuxInfoMaterialService = Arc<dyn AuxiliaryMaterialService<EcdsaAuxInfo>>;

#[async_trait]
pub(crate) trait AuxiliaryMaterialService<T>: Send + Sync + 'static {
    async fn lookup(&self, version: u32) -> Result<T, BlobRepositoryError>;
    async fn upsert(&self, version: u32, material: T) -> anyhow::Result<()>;
}

pub(crate) struct DefaultAuxiliaryMaterialService<T> {
    cached_material: Mutex<HashMap<u32, T>>,
    blob_service: Box<dyn BlobService<T>>,
}

impl<T> DefaultAuxiliaryMaterialService<T> {
    pub(crate) fn new(blob_service: Box<dyn BlobService<T>>) -> Self {
        Self { cached_material: Default::default(), blob_service }
    }

    fn key(version: u32) -> String {
        format!("v{version}")
    }
}

#[async_trait]
impl<T> AuxiliaryMaterialService<T> for DefaultAuxiliaryMaterialService<T>
where
    T: Clone + Send + Sync + 'static,
{
    async fn lookup(&self, version: u32) -> Result<T, BlobRepositoryError> {
        if let Some(material) = self.cached_material.lock().ok().and_then(|m| m.get(&version).cloned()) {
            return Ok(material);
        }

        let result = self.blob_service.find_one(&Self::key(version)).await;
        #[allow(clippy::expect_used)]
        if let Ok(material) = &result {
            // SAFETY: we can never panic while holding lock
            self.cached_material.lock().expect("lock poisoned").insert(version, material.clone());
        }
        result
    }

    async fn upsert(&self, version: u32, material: T) -> anyhow::Result<()> {
        self.blob_service.upsert(&Self::key(version), material).await?;
        Ok(())
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub(crate) trait AuxiliaryMaterialMetadataService: Send + Sync + 'static {
    async fn versions(
        &self,
        materials: &[AuxiliaryMaterial],
    ) -> Result<HashMap<AuxiliaryMaterial, u32>, MetadataLookupError>;
}

/// A service that provides auxiliary material metadata.
pub(crate) struct DefaultAuxiliaryMaterialMetadataService {
    repo: Arc<dyn AuxiliaryMaterialMetadataRepository>,
}

impl DefaultAuxiliaryMaterialMetadataService {
    pub(crate) fn new(repo: Arc<dyn AuxiliaryMaterialMetadataRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl AuxiliaryMaterialMetadataService for DefaultAuxiliaryMaterialMetadataService {
    async fn versions(
        &self,
        materials: &[AuxiliaryMaterial],
    ) -> Result<HashMap<AuxiliaryMaterial, u32>, MetadataLookupError> {
        // TODO do some caching
        let found = self.repo.find_all(materials).await?;
        let mapped = found
            .into_iter()
            .map(|AuxiliaryMaterialMetadata { material, generated_version }| (material, generated_version))
            .collect();
        Ok(mapped)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum MetadataLookupError {
    #[error("database: {0}")]
    Database(#[from] DatabaseError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::blob::MockBlobService;
    use mockall::predicate::eq;

    struct Builder {
        blob_service: MockBlobService<u32>,
    }

    impl Default for Builder {
        fn default() -> Self {
            Self { blob_service: Default::default() }
        }
    }

    impl Builder {
        fn build(self) -> DefaultAuxiliaryMaterialService<u32> {
            let Self { blob_service } = self;
            DefaultAuxiliaryMaterialService::new(Box::new(blob_service))
        }
    }

    #[tokio::test]
    async fn lookup_non_existent() {
        let version = 1;
        let version_str = "v1";
        let mut builder = Builder::default();
        builder
            .blob_service
            .expect_find_one()
            .with(eq(version_str))
            .return_once(|_| Err(BlobRepositoryError::NotFound));

        let err = builder.build().lookup(version).await.expect_err("lookup succeeded");
        assert!(matches!(err, BlobRepositoryError::NotFound), "not a not found error: {err:?}");
    }

    #[tokio::test]
    async fn lookup_is_cached() {
        let version = 1;
        let version_str = "v1";
        let mut builder = Builder::default();
        builder.blob_service.expect_find_one().with(eq(version_str)).return_once(|_| Ok(42));

        let service = builder.build();
        assert_eq!(service.lookup(version).await.expect("lookup failed"), 42);
        assert_eq!(service.lookup(version).await.expect("lookup failed"), 42);
    }

    #[tokio::test]
    async fn upsert() {
        let version = 1;
        let version_str = "v1";
        let value = 42;
        let mut builder = Builder::default();
        builder.blob_service.expect_upsert().with(eq(version_str), eq(value)).return_once(|_, _| Ok(()));

        builder.build().upsert(version, value).await.expect("lookup failed");
    }
}
