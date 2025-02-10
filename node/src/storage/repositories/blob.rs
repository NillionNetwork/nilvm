use async_trait::async_trait;
use bincode::Options;
use bytes::Bytes;
use object_store::{
    path::Path as ObjectStorePath, Error as ObjectStoreError, ObjectStore, PutMode, PutOptions, PutPayload,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{self, ErrorKind},
    marker::PhantomData,
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    fs::{self},
    io::AsyncWriteExt,
    sync::Mutex,
};

fn bincode_options() -> impl bincode::Options {
    #[allow(clippy::arithmetic_side_effects)]
    bincode::options()
        // Allow trailing bytes so the sender can be more up to date in the protocol structure
        // than us.
        .allow_trailing_bytes()
        // Varint encoding so messages are smaller.
        .with_varint_encoding()
        // Little endian because that's what we likely use anyway.
        .with_little_endian()
}

pub(crate) trait BinarySerde: Sized + Send + Sync + 'static {
    fn serialize(self) -> Result<Vec<u8>, BinarySerdeError>;
    fn deserialize(bytes: &[u8]) -> Result<Self, BinarySerdeError>;
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(crate) struct BinarySerdeError(pub(crate) String);

impl<T> BinarySerde for T
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn serialize(self) -> Result<Vec<u8>, BinarySerdeError> {
        bincode_options().serialize(&self).map_err(|e| BinarySerdeError(e.to_string()))
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, BinarySerdeError> {
        bincode_options().deserialize(bytes).map_err(|e| BinarySerdeError(e.to_string()))
    }
}

#[async_trait]
pub(crate) trait BlobRepository<T>: Send + Sync + 'static {
    /// Create new object.
    ///
    /// This will fail if the object already exists.
    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError>;

    /// Create new object or overwrite an existing one
    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError>;

    /// Read an object
    async fn read(&self, key: &str) -> Result<T, BlobRepositoryError>;

    /// Delete an object
    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError>;

    /// Checks the permissions on the underlying storage.
    async fn check_permissions(&self) -> Result<(), BlobRepositoryError>;
}

pub(crate) struct MemoryBlobRepository<T>(Arc<Mutex<HashMap<String, T>>>);

impl<T> Default for MemoryBlobRepository<T> {
    fn default() -> Self {
        MemoryBlobRepository(Default::default())
    }
}

#[async_trait]
impl<T: BinarySerde + Clone> BlobRepository<T> for MemoryBlobRepository<T> {
    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let mut entries = self.0.lock().await;
        if entries.contains_key(key) {
            return Err(BlobRepositoryError::AlreadyExists);
        }
        entries.insert(key.to_string(), value);
        Ok(())
    }

    #[allow(clippy::expect_used)]
    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        self.0.lock().await.insert(key.to_string(), value);
        Ok(())
    }

    #[allow(clippy::expect_used)]
    async fn read(&self, key: &str) -> Result<T, BlobRepositoryError> {
        self.0.lock().await.get(key).cloned().ok_or(BlobRepositoryError::NotFound)
    }

    #[allow(clippy::expect_used)]
    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError> {
        self.0.lock().await.remove(key);
        Ok(())
    }

    async fn check_permissions(&self) -> Result<(), BlobRepositoryError> {
        Ok(())
    }
}

/// A filesystem based blob repository.
pub struct FilesystemBlobRepository<T>(PathBuf, PhantomData<T>);

impl<T: BinarySerde> FilesystemBlobRepository<T> {
    /// Constructs a new filesystem backend that persists file in the specified path.
    pub fn new(path: PathBuf) -> Self {
        Self(path, PhantomData)
    }

    fn key_path(&self, key: &str) -> PathBuf {
        let clean_key = key.trim_start_matches('/');
        self.0.join(clean_key)
    }

    async fn put(&self, key: &str, value: T, opts: fs::OpenOptions) -> Result<(), BlobRepositoryError> {
        let path = self.key_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = value.serialize().map_err(BlobRepositoryError::Encode)?;
        let mut file = opts.open(path).await.map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                BlobRepositoryError::AlreadyExists
            } else {
                BlobRepositoryError::Io(e)
            }
        })?;
        file.write_all(&data).await?;
        file.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl<T: BinarySerde> BlobRepository<T> for FilesystemBlobRepository<T> {
    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let mut opts = fs::File::options();
        opts.create_new(true).write(true);
        self.put(key, value, opts).await
    }

    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let mut opts = fs::File::options();
        opts.create(true).write(true).truncate(true);
        self.put(key, value, opts).await
    }

    async fn read(&self, key: &str) -> Result<T, BlobRepositoryError> {
        let path = self.key_path(key);
        let data = fs::read(path).await?;
        let value = T::deserialize(&data).map_err(BlobRepositoryError::Decode)?;
        Ok(value)
    }

    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError> {
        let path = self.key_path(key);
        let result = fs::remove_file(path).await.map_err(BlobRepositoryError::from);
        // Ignore not found errors. S3 doesn't report this as an error and we want all repositories
        // to behave the same.
        if let Err(BlobRepositoryError::NotFound) = result { Ok(()) } else { result }
    }

    async fn check_permissions(&self) -> Result<(), BlobRepositoryError> {
        Ok(())
    }
}

#[allow(dead_code)]
pub struct ObjectStoreRepository<T> {
    object_store: Box<dyn ObjectStore>,
    _marker: PhantomData<T>,
}

impl<T: BinarySerde> ObjectStoreRepository<T> {
    #[allow(dead_code)]
    pub(crate) fn new(object_store: Box<dyn ObjectStore>) -> Self {
        Self { object_store, _marker: PhantomData }
    }

    async fn put(&self, key: &str, value: T, opts: PutOptions) -> Result<(), BlobRepositoryError> {
        let path = ObjectStorePath::parse(key).map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        let data = value.serialize().map_err(BlobRepositoryError::Encode)?;
        let payload = PutPayload::from_bytes(Bytes::copy_from_slice(&data));
        match self.object_store.put_opts(&path, payload, opts).await {
            Ok(_) => Ok(()),
            Err(object_store::Error::AlreadyExists { .. }) => Err(BlobRepositoryError::AlreadyExists),
            Err(e) => Err(BlobRepositoryError::Internal(e.to_string())),
        }
    }
}

#[async_trait]
impl<T: BinarySerde> BlobRepository<T> for ObjectStoreRepository<T> {
    async fn create(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let opts = PutOptions { mode: PutMode::Create, ..Default::default() };
        self.put(key, value, opts).await
    }

    async fn upsert(&self, key: &str, value: T) -> Result<(), BlobRepositoryError> {
        let opts = Default::default();
        self.put(key, value, opts).await
    }

    async fn read(&self, key: &str) -> Result<T, BlobRepositoryError> {
        let path = ObjectStorePath::parse(key).map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        let response = match self.object_store.get(&path).await {
            Ok(resp) => resp,
            Err(e) => return Err(e.into()),
        };
        let object: Bytes = response.bytes().await.map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        let value = T::deserialize(&object).map_err(BlobRepositoryError::Decode)?;
        Ok(value)
    }

    async fn delete(&self, key: &str) -> Result<(), BlobRepositoryError> {
        let path = ObjectStorePath::parse(key).map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        if let Err(e) = self.object_store.delete(&path).await {
            match e {
                ObjectStoreError::NotFound { .. } => return Ok(()),
                _ => return Err(e.into()),
            }
        }
        Ok(())
    }

    async fn check_permissions(&self) -> Result<(), BlobRepositoryError> {
        let key = "check-permissions";
        let path = ObjectStorePath::parse(key).map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        let payload = PutPayload::from_bytes(Bytes::copy_from_slice(&[42]));
        self.object_store.put(&path, payload).await.map_err(|e| BlobRepositoryError::Internal(e.to_string()))?;
        self.object_store.get(&path).await?;
        self.object_store.delete(&path).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum BlobRepositoryError {
    #[error("not found")]
    NotFound,

    #[error("io: {0}")]
    Io(io::Error),

    #[error("internal: {0}")]
    Internal(String),

    #[error("entry already exists")]
    AlreadyExists,

    #[error("encoding: {0}")]
    Encode(BinarySerdeError),

    #[error("decoding: {0}")]
    Decode(BinarySerdeError),
}

impl From<io::Error> for BlobRepositoryError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound { Self::NotFound } else { Self::Io(error) }
    }
}

impl From<ObjectStoreError> for BlobRepositoryError {
    fn from(error: ObjectStoreError) -> Self {
        match error {
            ObjectStoreError::NotFound { .. } => Self::NotFound,
            _ => Self::Internal(error.to_string()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use object_store::{
        aws::{AmazonS3Builder, S3ConditionalPut},
        gcp::GoogleCloudStorageBuilder,
        ClientOptions,
    };
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use rstest::*;
    use serde_json::json;
    use std::{env, env::temp_dir, fs::create_dir, ops::Deref, sync::Once};
    use testcontainers::{
        core::{Mount, WaitFor},
        runners::AsyncRunner,
        ContainerAsync, GenericImage, ImageExt,
    };
    use tokio::fs::create_dir_all;

    static LOGGER_INIT: Once = Once::new();
    #[ctor::ctor]
    fn init_logging() {
        LOGGER_INIT.call_once(|| {
            env_logger::builder().is_test(true).init();
        });
    }

    const S3_BUCKET_PREFIX: &str = "s3-repository-test";
    const S3_REGION: &str = "eu-west-1";
    const S3_CONTAINER_PREFIX: &str = "nillion-s3-repository-test";
    const S3_DEFAULT_TESTCONTAINERS_MINIO_IMAGE: &str = "minio/minio:latest";
    const GCS_CONTAINER_PREFIX: &str = "nillion-gcs-repository-test";
    const GCS_BUCKET_PREFIX: &str = "gcs-repository-test";
    const GCS_DEFAULT_TESTCONTAINERS_GCS_SERVER_IMAGE: &str = "fsouza/fake-gcs-server:latest";

    struct TestRepository {
        repo: Box<dyn BlobRepository<u32>>,
        _container: Option<ContainerAsync<GenericImage>>,
    }

    impl TestRepository {
        pub fn new(repo: Box<dyn BlobRepository<u32>>) -> Self {
            TestRepository { repo, _container: None }
        }
        pub fn new_with_container(repo: Box<dyn BlobRepository<u32>>, container: ContainerAsync<GenericImage>) -> Self {
            TestRepository { repo, _container: Some(container) }
        }
    }

    impl Deref for TestRepository {
        type Target = dyn BlobRepository<u32>;
        fn deref(&self) -> &Self::Target {
            &*self.repo
        }
    }

    fn make_memory_repository() -> TestRepository {
        TestRepository::new(Box::new(MemoryBlobRepository::default()))
    }

    fn make_filesystem_repository() -> TestRepository {
        let path = temp_dir().join(format!("test-filesystem-repository-{}", random_string()));
        create_dir(&path).expect("failed to create directory");
        TestRepository::new(Box::new(FilesystemBlobRepository::new(path)))
    }

    async fn make_object_store_s3_repository() -> TestRepository {
        let container_image = env::var("S3_TESTCONTAINERS_MINIO_IMAGE")
            .unwrap_or_else(|_| S3_DEFAULT_TESTCONTAINERS_MINIO_IMAGE.to_string());
        let (container_image, image_tag) = container_image.rsplit_once(':').unwrap_or((&container_image, "latest"));
        let bucket_name = format!("{}-{}", S3_BUCKET_PREFIX, random_string().to_lowercase());

        let container_name = format!("{}-{}", S3_CONTAINER_PREFIX, random_string().to_lowercase());
        let container = GenericImage::new(container_image, &*image_tag)
            .with_entrypoint("sh")
            .with_wait_for(WaitFor::message_on_stderr("API:"))
            .with_env_var("MINIO_CONSOLE_ADDRESS", ":9001")
            .with_cmd(vec!["-c", &format!("mkdir -p /data/{bucket_name} && /usr/bin/minio server /data")])
            .with_container_name(container_name)
            .start()
            .await
            .expect("failed to start S3 container");

        let s3_host = container.get_host().await.expect("failed to get S3 container host").to_string();
        let s3_port = container.get_host_port_ipv4(9000).await.expect("failed to get S3 container port");

        let s3 = AmazonS3Builder::new()
            .with_region(S3_REGION)
            .with_endpoint(format!("http://{}:{}", s3_host, s3_port))
            .with_allow_http(true)
            .with_bucket_name(bucket_name)
            .with_access_key_id("minioadmin")
            .with_secret_access_key("minioadmin")
            .with_conditional_put(S3ConditionalPut::ETagMatch)
            .build()
            .expect("create aws s3 client");
        let object_store = Box::new(s3);
        TestRepository::new_with_container(Box::new(ObjectStoreRepository::<u32>::new(object_store)), container)
    }

    async fn make_object_store_gcs_repository() -> TestRepository {
        let bucket_name = format!("{}-{}", GCS_BUCKET_PREFIX, random_string().to_lowercase());
        let tmp = temp_dir();
        let data_dir = tmp.join("fake-gcs-server").join(random_string().to_lowercase());
        let bucket_dir = data_dir.join(bucket_name.clone());
        create_dir_all(&bucket_dir).await.expect("failed to create bucket directory");

        let container_image = env::var("TESTCONTAINERS_GCS_SERVER_IMAGE")
            .unwrap_or_else(|_| GCS_DEFAULT_TESTCONTAINERS_GCS_SERVER_IMAGE.to_string());
        let (container_image, image_tag) = container_image.rsplit_once(':').unwrap_or((&container_image, "latest"));

        let container_name = format!("{}-{}", GCS_CONTAINER_PREFIX, random_string().to_lowercase());
        let container = GenericImage::new(container_image, &*image_tag)
            .with_wait_for(WaitFor::message_on_stderr("server started at"))
            .with_container_name(container_name)
            .with_mount(Mount::bind_mount(data_dir.to_string_lossy(), "/data"))
            .start()
            .await
            .expect("failed to start GCS container");

        let gcs_host = container.get_host().await.expect("failed to get S3 container host").to_string();
        let gcs_port = container.get_host_port_ipv4(4443).await.expect("failed to get S3 container port");
        let gcs_url = format!("https://{}:{}/storage/v1/b", gcs_host, gcs_port);

        let service_account_key = json!({
            "type": "service_account",
            "project_id": "project_id",
            "private_key_id": "private_key_id",
            "private_key": "private_key",
            "client_email": "nillion-gcs-test-bucket@project_id.iam.gserviceaccount.com",
            "client_id": "client_id",
            "disable_oauth":true,
            "gcs_base_url": gcs_url
        });

        let gcs = GoogleCloudStorageBuilder::new()
            .with_bucket_name(bucket_name)
            .with_service_account_key(service_account_key.to_string())
            .with_client_options(ClientOptions::new().with_allow_invalid_certificates(true))
            .build()
            .expect("create gcs client");

        let object_store = Box::new(gcs);
        TestRepository::new_with_container(Box::new(ObjectStoreRepository::<u32>::new(object_store)), container)
    }

    async fn make_object_store_real_gcs_repository() -> TestRepository {
        let bucket_name = "nillion-gcs-test-bucket";
        let service_account_path =
            env::var("GCS_SERVICE_ACCOUNT_PATH").expect("GCS_SERVICE_ACCOUNT_PATH env var is required");
        let gcs = GoogleCloudStorageBuilder::new()
            .with_bucket_name(bucket_name)
            .with_service_account_path(service_account_path)
            .build()
            .expect("create gcs client");

        let object_store = Box::new(gcs);
        TestRepository::new(Box::new(ObjectStoreRepository::<u32>::new(object_store)))
    }

    fn random_string() -> String {
        thread_rng().sample_iter(&Alphanumeric).take(10).map(char::from).collect()
    }

    #[rstest]
    #[case::memory(make_memory_repository())]
    #[case::filesystem(make_filesystem_repository())]
    #[test_with::no_env(DISABLE_S3_BLOB_REPOSITORY_TESTS)]
    #[case::object_store_s3(make_object_store_s3_repository().await)]
    #[test_with::env(GCS_SERVICE_ACCOUNT_PATH)]
    #[case::object_store_real_gcs(make_object_store_real_gcs_repository().await)]
    #[ignore]
    #[case::object_store_local_gcs(make_object_store_gcs_repository().await)]
    #[tokio::test]
    async fn test_delete_nonexistent(#[case] repo: TestRepository) {
        // we expect deleting non-existent keys to return Ok
        repo.delete("non-existent-key").await.expect("deletion failed");
    }

    #[rstest]
    #[case::memory(make_memory_repository())]
    #[case::filesystem(make_filesystem_repository())]
    #[test_with::no_env(DISABLE_S3_BLOB_REPOSITORY_TESTS)]
    #[case::object_store_s3(make_object_store_s3_repository().await)]
    #[test_with::env(GCS_SERVICE_ACCOUNT_PATH)]
    #[case::object_store_real_gcs(make_object_store_real_gcs_repository().await)]
    #[ignore]
    #[case::object_store_local_gcs(make_object_store_gcs_repository().await)]
    #[tokio::test]
    async fn test_bucket_access(#[case] repo: TestRepository) {
        let bucket_check = repo.check_permissions().await;
        assert!(matches!(bucket_check, Ok(())), "{bucket_check:?}");
    }

    #[rstest]
    #[case::memory(make_memory_repository())]
    #[case::filesystem(make_filesystem_repository())]
    #[test_with::no_env(DISABLE_S3_BLOB_REPOSITORY_TESTS)]
    #[case::object_store_s3(make_object_store_s3_repository().await)]
    #[test_with::env(GCS_SERVICE_ACCOUNT_PATH)]
    #[case::object_store_real_gcs(make_object_store_real_gcs_repository().await)]
    #[ignore]
    #[case::object_store_local_gcs(make_object_store_gcs_repository().await)]
    #[tokio::test]
    async fn test_crud(#[case] repo: TestRepository) {
        let data = 42;
        let key_str = format!("/random_{}.bin", random_string());
        let key = key_str.as_str();

        repo.upsert(key, data).await.expect("failed to create object");
        let retrieved_data = repo.read(key).await.expect("failed to read object");
        assert_eq!(data, retrieved_data);

        let err = repo.create(key, data).await.expect_err("creating object succeeded");
        assert!(matches!(err, BlobRepositoryError::AlreadyExists), "{err:?}");

        let data = 1337;
        repo.upsert(key, data).await.expect("failed to update object");
        let retrieved_data = repo.read(key).await.expect("failed to read object");
        assert_eq!(data, retrieved_data);

        repo.delete(key).await.expect("failed to delete object");
        repo.read(key).await.expect_err("reading object succeeed");
    }

    #[rstest]
    #[case::memory(make_memory_repository())]
    #[case::filesystem(make_filesystem_repository())]
    #[test_with::no_env(DISABLE_S3_BLOB_REPOSITORY_TESTS)]
    #[case::object_store_s3(make_object_store_s3_repository().await)]
    #[test_with::env(GCS_SERVICE_ACCOUNT_PATH)]
    #[case::object_store_real_gcs(make_object_store_real_gcs_repository().await)]
    #[ignore]
    #[case::object_store_local_gcs(make_object_store_gcs_repository().await)]
    #[tokio::test]
    async fn test_list_keys(#[case] repo: TestRepository) {
        let root_folder = format!("/test_s3_backends/test_list_keys/random_{}", random_string());
        let keys = vec![
            format!("{}/folder1/1.bin", root_folder),
            format!("{}/folder1/2.bin", root_folder),
            format!("{}/folder1/folder2/1.bin", root_folder),
            format!("{}/banana", root_folder),
        ];

        for key in keys.iter() {
            repo.upsert(key, 42).await.expect("Upsert failed");
        }

        for key in keys.iter() {
            repo.delete(key).await.expect("Delete failed");
        }
    }
}
