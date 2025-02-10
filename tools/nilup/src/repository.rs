#[cfg(target_os = "macos")]
use crate::{create_dir, run_command::run_command};
use eyre::Result;
use futures_util::StreamExt;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    path::Path as ObjectStorePath,
    ObjectStore,
};

use serde::Deserialize;
use std::{collections::HashSet, fmt::Display, path::PathBuf};
use tracing::warn;

#[derive(Deserialize, Clone)]
struct Packages {
    sdk_bins: String,
}

/// Manifest file structure
#[derive(Deserialize, Clone)]
#[allow(dead_code)]
struct Manifest {
    linux_amd64: Packages,
    linux_aarch64: Packages,
    macos_amd64: Packages,
    macos_aarch64: Packages,
}

impl Manifest {
    fn get_packages(self) -> Packages {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return self.linux_amd64;
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return self.linux_aarch64;
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return self.macos_amd64;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return self.macos_aarch64;
    }
}

/// Channel
#[derive(Clone)]
enum Channel {
    Stable,
    Rc,
}

impl Channel {
    fn prefix(&self) -> &'static str {
        match self {
            Channel::Stable => "public/sdk/",
            Channel::Rc => "public/sdk-rc/",
        }
    }
}

impl Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prefix())
    }
}

/// Repository that host the Nillion SDK
pub struct Repository {
    client: AmazonS3,
}

impl Repository {
    pub async fn new() -> Self {
        let client = AmazonS3Builder::from_env()
            .with_bucket_name("nillion-releases")
            .with_region("eu-west-1")
            .with_skip_signature(true)
            .build()
            .expect("Failed to create S3 client");

        Repository { client }
    }

    /// Checks if a version exists in the repository
    /// first checks in the stable releases then in the release candidates
    pub async fn version_exist(&self, version: impl Into<String>) -> Result<bool> {
        let version = version.into();
        let stable_path = ObjectStorePath::parse(&format!("{}{version}", Channel::Stable))?;
        let mut response = self.client.list(Some(&stable_path));
        if response.next().await.is_some() {
            return Ok(true);
        }
        let rc_path = ObjectStorePath::parse(&format!("{}{version}", Channel::Rc))?;
        let mut response = self.client.list(Some(&rc_path));

        Ok(response.next().await.is_some())
    }

    /// List all the versions available in the repository
    /// if rc is true, it will list the release candidates instead of stable
    pub async fn list_versions(&self, rc: bool) -> Result<Vec<String>> {
        let mut versions = HashSet::new();

        let channel = if rc { Channel::Rc } else { Channel::Stable };
        let path = ObjectStorePath::parse(channel.prefix())?;
        let mut response = self.client.list(Some(&path));

        while let Some(result) = response.next().await {
            match result {
                Ok(response) => {
                    if let Some(version) = response.location.as_ref().replace(channel.prefix(), "").split('/').next() {
                        versions.insert(version.to_string());
                    }
                }
                Err(err) => {
                    warn!("Error listing objects: {:?}", err);
                    break;
                }
            }
        }

        let mut versions = versions.into_iter().collect::<Vec<String>>();
        versions.sort_by_key(|e| e.to_lowercase());
        Ok(versions)
    }

    /// Get the manifest file for a version
    /// first checks in the stable releases then in the release candidates
    async fn get_manifest(&mut self, version: &str) -> Result<(Channel, Manifest)> {
        let response = self.try_find_manifest(Channel::Stable, version).await;
        if let Ok(response) = response {
            return Ok(response);
        }
        self.try_find_manifest(Channel::Rc, version).await
    }

    /// Try to find the manifest file for a version in a channel
    async fn try_find_manifest(&self, channel: Channel, version: &str) -> Result<(Channel, Manifest)> {
        let key = format!("{version}/manifest.yaml");
        let channel_path = ObjectStorePath::parse(format!("{}{key}", channel))?;
        let response = self.client.get(&channel_path).await?;
        let data = response.bytes().await?;
        let manifest: Manifest = serde_yaml::from_slice(&data)?;
        Ok((channel, manifest))
    }

    /// Download the SDK binaries for a version
    pub async fn download_sdk_bins(&mut self, version: &str, path: &PathBuf) -> Result<()> {
        let (channel, manifest) = self.get_manifest(version).await?;
        let sdk_bins = manifest.get_packages().sdk_bins;

        let key = format!("{version}/{sdk_bins}");

        println!("Downloading {}", key);

        let object_path = ObjectStorePath::parse(format!("{channel}{key}"))?;

        let response = self.client.get(&object_path).await?;

        let data = response.bytes().await?;

        #[cfg(target_os = "linux")]
        {
            use flate2::read::GzDecoder;
            use tar::Archive;
            // create a reader buffer with data

            let tar = GzDecoder::new(data.as_ref());
            let mut archive = Archive::new(tar);
            archive.unpack(path)?;
        }
        #[cfg(target_os = "macos")]
        {
            use tokio::io::AsyncWriteExt;
            create_dir(path).await?;
            let volume_name = "nillion-sdk";
            let volume_path = format!("/Volumes/{volume_name}");
            let dmg_path = format!("/tmp/NillionSDK-{version}.dmg");
            let mut file = tokio::fs::File::create(dmg_path.clone()).await?;
            file.write_all(data.as_ref()).await?;
            file.sync_all().await?;

            run_command("hdiutil", &["attach", &dmg_path]).await?;

            let result = run_command("cp", &["-r", &format!("{volume_path}/."), path.to_string_lossy().as_ref()]).await;

            run_command("diskutil", &["unmountDisk", &volume_path]).await?;
            run_command("diskutil", &["eject", volume_name]).await?;

            result?;
        }

        Ok(())
    }
}
