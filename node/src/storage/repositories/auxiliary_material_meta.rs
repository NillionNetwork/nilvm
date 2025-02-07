use crate::storage::{
    metrics::ExportMetrics,
    sqlite::{DatabaseError, SqliteDb},
};
use async_trait::async_trait;
use itertools::Itertools;
use node_api::preprocessing::rust::AuxiliaryMaterial;
use sqlx::FromRow;
use std::iter;

#[derive(Clone, Debug, PartialEq, FromRow)]
pub(crate) struct AuxiliaryMaterialMetadata {
    #[sqlx(try_from = "&'a str")]
    pub(crate) material: AuxiliaryMaterial,

    pub(crate) generated_version: u32,
}

#[async_trait]
pub(crate) trait AuxiliaryMaterialMetadataRepository: Send + Sync + 'static {
    async fn find_all(&self, materials: &[AuxiliaryMaterial]) -> Result<Vec<AuxiliaryMaterialMetadata>, DatabaseError>;
    async fn find(&self, material: AuxiliaryMaterial) -> Result<Option<AuxiliaryMaterialMetadata>, DatabaseError>;
    async fn insert(&self, meta: AuxiliaryMaterialMetadata) -> Result<(), DatabaseError>;
}

pub(crate) struct SqliteAuxiliaryMaterialMetadataRepository(SqliteDb);

impl SqliteAuxiliaryMaterialMetadataRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self(db)
    }
}

#[async_trait]
impl AuxiliaryMaterialMetadataRepository for SqliteAuxiliaryMaterialMetadataRepository {
    async fn find_all(&self, materials: &[AuxiliaryMaterial]) -> Result<Vec<AuxiliaryMaterialMetadata>, DatabaseError> {
        if materials.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = iter::repeat("?").take(materials.len()).join(",");
        let sql_query = format!("SELECT * FROM auxiliary_material_metadata WHERE material IN ({placeholders})");
        let mut query = sqlx::query_as(&sql_query);
        for material in materials {
            query = query.bind(material.to_string());
        }
        self.0.fetch_all(query, &mut Default::default()).await
    }

    async fn find(&self, material: AuxiliaryMaterial) -> Result<Option<AuxiliaryMaterialMetadata>, DatabaseError> {
        let query =
            sqlx::query_as("SELECT * FROM auxiliary_material_metadata WHERE material = ?").bind(material.to_string());
        self.0.fetch_one(query, &mut Default::default()).await
    }

    async fn insert(&self, meta: AuxiliaryMaterialMetadata) -> Result<(), DatabaseError> {
        let query = sqlx::query("INSERT INTO auxiliary_material_metadata (material, generated_version) VALUES (?, ?)")
            .bind(meta.material.to_string())
            .bind(meta.generated_version);
        self.0.execute(query, &mut Default::default()).await.map(|_| ())
    }
}

#[async_trait]
impl ExportMetrics for SqliteAuxiliaryMaterialMetadataRepository {
    async fn export_metrics(&self) -> anyhow::Result<()> {
        self.0.export_table_metrics("auxiliary_material_metadata").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_repo() -> SqliteAuxiliaryMaterialMetadataRepository {
        let handle = SqliteDb::new("sqlite::memory:").await.expect("repo creation failed");
        SqliteAuxiliaryMaterialMetadataRepository::new(handle)
    }

    #[tokio::test]
    async fn find_non_existent() {
        let repo = make_repo().await;
        let result = repo.find(AuxiliaryMaterial::Cggmp21AuxiliaryInfo).await.expect("query failed");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn find_existing() {
        let repo = make_repo().await;
        let meta =
            AuxiliaryMaterialMetadata { material: AuxiliaryMaterial::Cggmp21AuxiliaryInfo, generated_version: 42 };
        repo.insert(meta.clone()).await.expect("insert failed");
        let result = repo.find(AuxiliaryMaterial::Cggmp21AuxiliaryInfo).await.expect("query failed");
        assert_eq!(result, Some(meta));
    }
}
