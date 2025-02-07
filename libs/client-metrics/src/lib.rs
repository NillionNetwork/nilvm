//! This library provides a way to track client metrics using Piwik.
//! This tracking is based on the tracking files in the user's home directory.
//! The $HOME/.nillion/tracking directory should contain the following files:
//! - enabled: a file that enables the tracking
//! - tracking_id: a file containing the 16-character track id
//! - wallet_addr: an optional file containing the 42-character wallet address
//!
//! The tracking is disabled by default.
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

use anyhow::{anyhow, Context, Result};
use build_info::BuildInfo;
use piwik_track_client::{PiwikClient, TrackEvent};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, future, path::PathBuf, sync::Arc};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

mod piwik_track_client;

const PIWIK_SITE_ID: &str = "9a094e78-9ef7-4c66-959c-fb0cc3c78c6c";
const PIWIK_INSTANCE_NAME: &str = "nillion";

/// Creates a hashmap from the fields.
#[macro_export]
macro_rules! fields {
    {$($k: expr => $v: expr),* $(,)?} => {
        {
            #[allow(dead_code)]
            {
                $(
                    let _ = $crate::MustImplementToString(&$k);
                    let _ = $crate::MustImplementToString(&$v);
                )*
            }
            Some(::std::collections::HashMap::from([$(($k.to_string(), $v.to_string()),)*]))
        }
    };
}

/// A struct to enforce the implementation of `ToString` in the fields! macro values.
/// It helps to give a better error message in the fields! macro.
#[allow(dead_code)]
pub struct MustImplementToString<T: ToString>(pub T);

#[derive(Serialize, Deserialize)]
struct Configuration {
    enabled: bool,
    tracking_id: String,
    wallet_address: Option<String>,
}

/// Client metrics client.
#[derive(Clone)]
pub struct Client {
    tracking_id: String,
    wallet_addr: Option<String>,
    bin_name: String,
    commit_version: String,
    client: Arc<PiwikClient>,
}

/// Client metrics client.
/// It can be enabled or disabled.
/// If enabled, it will send client metrics events.
/// If disabled, it will not send any events.
#[derive(Clone)]
pub enum ClientMetrics {
    /// Enabled client metrics.
    Enabled(Client),
    /// Disabled client metrics.
    Disabled,
}

impl ClientMetrics {
    /// Creates a new client metrics instance.
    /// If the tracking is enabled, it will return a `ClientMetrics::Enabled` with the client metrics instance.
    /// If the tracking is disabled, it will return a `ClientMetrics::Disabled`.
    /// # Arguments
    /// * `instance_name` - The Piwik instance name.
    /// * `site_id` - The Piwik site id.
    /// * `bin_name` - The name of the binary.
    /// * `commit_version` - The commit version of the binary.
    pub fn new(
        instance_name: String,
        site_id: String,
        bin_name: String,
        commit_version: String,
    ) -> Result<ClientMetrics> {
        let configuration: Configuration = Self::get_configuration();
        if configuration.enabled {
            debug!("Client metrics enabled");
            Ok(ClientMetrics::Enabled(Client {
                tracking_id: configuration.tracking_id,
                wallet_addr: configuration.wallet_address,
                bin_name,
                commit_version,
                client: Arc::new(PiwikClient::new(instance_name, site_id)?),
            }))
        } else {
            debug!("Client metrics disabled");
            Ok(ClientMetrics::Disabled)
        }
    }

    /// Creates a new client metrics instance with the default Piwik instance name and site id.
    /// If the tracking is enabled, it will return a `ClientMetrics::Enabled` with the client metrics instance.
    /// If the tracking is disabled, it will return a `ClientMetrics::Disabled`.
    /// # Arguments
    /// * `bin_name` - The name of the binary.
    pub fn new_default<B: ToString>(bin_name: B) -> ClientMetrics {
        let commit_version = BuildInfo::default().git_commit_hash;
        let result = Self::new(
            PIWIK_INSTANCE_NAME.to_string(),
            PIWIK_SITE_ID.to_string(),
            bin_name.to_string(),
            commit_version.to_string(),
        );
        result.unwrap_or_else(|e| {
            warn!("Error creating client metrics: {}", e);
            ClientMetrics::Disabled
        })
    }

    /// Generates a random 16 numbers track id.
    fn generate_tracking_id() -> String {
        let min = 10u64.pow(15);
        let max = 10u64.pow(16);
        let number = thread_rng().gen_range(min..max);
        number.to_string()
    }

    /// Gets the configuration, if not found, it will return a default configuration.
    fn get_configuration() -> Configuration {
        if let Ok(conf) = Self::read_configuration() {
            conf
        } else {
            debug!("Configuration not found");
            Configuration { enabled: false, tracking_id: "".to_string(), wallet_address: None }
        }
    }

    /// Enables the client metrics tracking.
    /// # Arguments
    /// * `wallet_addr` - Optional wallet address to be tracked.
    pub fn enable(wallet_addr: Option<String>) -> Result<()> {
        debug!("Enabling client metrics");
        let conf = if let Ok(mut conf) = Self::read_configuration() {
            debug!("Configuration found");
            conf.enabled = true;
            if let Some(wallet_addr) = wallet_addr {
                conf.wallet_address = Some(wallet_addr);
            }
            conf
        } else {
            debug!("Configuration not found, creating new configuration");
            Configuration { enabled: true, tracking_id: Self::generate_tracking_id(), wallet_address: wallet_addr }
        };
        Self::save_configuration(&conf)
    }

    /// Disables the client metrics tracking.
    pub fn disable() -> Result<()> {
        debug!("Disabling client metrics");
        let mut conf = Self::read_configuration()?;
        conf.enabled = false;
        Self::save_configuration(&conf)?;
        Ok(())
    }

    /// Sends a client metric event.
    /// # Arguments
    /// * `command` - The command to be tracked.
    /// * `fields` - Optional fields to be tracked.
    /// # Returns
    /// A `JoinHandle` to the spawned task that sends the event.
    pub fn send_event<C: ToString>(&self, command: C, fields: Option<HashMap<String, String>>) -> JoinHandle<()> {
        self._send_event(command.to_string(), None, fields)
    }

    /// Sends a client metric error event.
    /// # Arguments
    /// * `command` - The command to be tracked.
    /// * `error` - The error message.
    /// * `fields` - Optional fields to be tracked.
    /// # Returns
    /// A `JoinHandle` to the spawned task that sends the event.
    pub fn send_error<C: ToString, E: ToString>(
        &self,
        command: C,
        error: E,
        fields: Option<HashMap<String, String>>,
    ) -> JoinHandle<()> {
        self._send_event(command.to_string(), Some(error.to_string()), fields)
    }

    /// Sends a client metric event.
    fn _send_event(
        &self,
        command: String,
        error: Option<String>,
        fields: Option<HashMap<String, String>>,
    ) -> JoinHandle<()> {
        if let ClientMetrics::Enabled(client) = self {
            debug!("Sending client metric event");
            let client = client.clone();
            let future = async move {
                let result: Result<()> = async {
                    let event = Self::create_track_event(&client, command, error, fields)?;
                    client.client.track(event).await?;
                    Ok(())
                }
                .await;
                if let Err(e) = result {
                    warn!("Error sending client metric: {}", e);
                }
            };

            tokio::spawn(future)
        } else {
            debug!("Not sending event because client metrics are disabled");
            tokio::spawn(future::ready(()))
        }
    }

    /// Creates a track event.
    fn create_track_event(
        client: &Client,
        command: String,
        error: Option<String>,
        fields: Option<HashMap<String, String>>,
    ) -> Result<TrackEvent> {
        let arch = std::env::consts::ARCH;
        let os_family = std::env::consts::FAMILY;
        let os = std::env::consts::OS;

        // piwik custom variables https://help.piwik.pro/analytics/custom-variables/
        let mut custom_vars: HashMap<String, (String, String)> = HashMap::new();
        custom_vars.insert("1".to_string(), ("os_family".to_string(), os_family.to_string()));
        custom_vars.insert("2".to_string(), ("arch".to_string(), arch.to_string()));
        custom_vars.insert("3".to_string(), ("os".to_string(), os.to_string()));
        custom_vars.insert("4".to_string(), ("bin_name".to_string(), client.bin_name.to_string()));
        custom_vars.insert("5".to_string(), ("commit_version".to_string(), client.commit_version.to_string()));
        custom_vars.insert("6".to_string(), ("command".to_string(), command.clone()));
        if let Some(wallet_addr) = &client.wallet_addr {
            custom_vars.insert("7".to_string(), ("wallet_addr".to_string(), wallet_addr.clone()));
        }
        if error.is_some() {
            custom_vars.insert("8".to_string(), ("error".to_string(), "true".to_string()));
        }
        let custom_vars = serde_json::to_string(&custom_vars)?;

        let mut fields = fields.unwrap_or_default();

        if let Some(error) = error {
            fields.insert("error".to_string(), error);
        }

        let fields_url_encoded = if fields.is_empty() {
            "".to_string()
        } else {
            let fields = serde_urlencoded::to_string(fields)?;
            format!("?{}", fields)
        };

        Ok(TrackEvent::new()
            ._id(client.tracking_id.clone())
            .url(format!("nilsdk://{}/{}/{}{}", client.bin_name, client.commit_version, command, fields_url_encoded))
            .ua(format!("Nillion Client Metrics ({} {})", os, arch))
            .action_name(format!("{}/{}", client.bin_name, command))
            .cvar(custom_vars))
    }
}

impl ClientMetrics {
    /// Save configuration to the tracking directory.
    fn save_configuration(conf: &Configuration) -> Result<()> {
        debug!("Saving configuration");
        let tracking_path = Self::tracking_path()?;
        if !tracking_path.exists() {
            debug!("Creating tracking directory");
            std::fs::create_dir_all(&tracking_path).context("Could not create tracking directory")?;
        }
        let tracking_config_path = tracking_path.join("configuration.toml");
        let conf_str = toml::to_string(&conf).context("Could not serialize configuration")?;
        std::fs::write(tracking_config_path, conf_str).context("Could not write configuration file")?;
        Ok(())
    }

    /// Get configuration from the tracking directory.
    fn read_configuration() -> Result<Configuration> {
        let tracking_path = Self::tracking_path()?;
        debug!("Reading configuration from: {:?}", tracking_path);
        let tracking_config_path = tracking_path.join("configuration.toml");
        let conf: Configuration = toml::from_str(
            &std::fs::read_to_string(tracking_config_path).context("Could not read configuration file")?,
        )
        .context("Could not deserialize configuration")?;
        Ok(conf)
    }

    /// Gets the tracking path.
    fn tracking_path() -> Result<PathBuf> {
        Ok(dirs::home_dir().ok_or(anyhow!("HOME dir not found"))?.join(".nillion").join("tracking"))
    }

    /// Sends a client metric event synchronously.
    /// # Arguments
    /// * `command` - The command to be tracked.
    /// * `fields` - Optional fields to be tracked.
    pub fn send_event_sync<C: ToString>(&self, command: C, fields: Option<HashMap<String, String>>) {
        match tokio::runtime::Runtime::new() {
            Ok(runtime) => match runtime.block_on(async { self.send_event(command, fields).await }) {
                Ok(_) => (),
                Err(e) => warn!("Error sending client metric: {}", e),
            },
            Err(e) => warn!("Error creating tokio runtime: {}", e),
        }
    }

    /// Sends a client metric error event synchronously.
    /// # Arguments
    /// * `command` - The command to be tracked.
    /// * `error` - The error message.
    /// * `fields` - Optional fields to be tracked.
    pub fn send_error_sync<C: ToString, E: ToString>(
        &self,
        command: C,
        error: E,
        fields: Option<HashMap<String, String>>,
    ) {
        match tokio::runtime::Runtime::new() {
            Ok(runtime) => match runtime.block_on(async { self.send_error(command, error, fields).await }) {
                Ok(_) => (),
                Err(e) => warn!("Error sending client metric: {}", e),
            },
            Err(e) => warn!("Error creating tokio runtime: {}", e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::{
        piwik_track_client::{
            test::{INSTANCE_NAME, SITE_ID},
            PiwikClient,
        },
        Client, ClientMetrics,
    };
    use std::sync::Arc;

    #[test]
    fn test() {
        let client = ClientMetrics::Enabled(Client {
            tracking_id: ClientMetrics::generate_tracking_id(),
            wallet_addr: None,
            bin_name: "nil-test".to_string(),
            commit_version: "ae3b42f".to_string(),
            client: Arc::new(PiwikClient::new(INSTANCE_NAME.to_string(), SITE_ID.to_string()).unwrap()),
        });
        let fields = fields! {
            "test-key" => "test-value"
        };

        client.send_event_sync("store".to_string(), fields.clone());
        client.send_error_sync("store".to_string(), "my test error".to_string(), fields);
    }

    #[test]
    fn test_track_id() {
        let track_id = ClientMetrics::generate_tracking_id();
        assert_eq!(track_id.len(), 16);
        track_id.parse::<u64>().unwrap();
    }
}
