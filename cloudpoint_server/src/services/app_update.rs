use anyhow::{Result, anyhow};
use cloudpoint_lib::preflight::ClientUpdate;
use semver::Version as SemVer;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::{sync::RwLock, task, time::interval};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppUpdater(Arc<RwLock<Option<ClientUpdate>>>);

#[derive(Deserialize)]
struct GhResponse {
    tag_name: SemVer,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    browser_download_url: String,
    name: String,
    // size: usize,
    // digest: String,
}

impl AppUpdater {
    pub fn new() -> Self {
        let inner = Arc::new(RwLock::new(None));
        let local_inner = Arc::clone(&inner);

        task::spawn(async move {
            let mut interval = interval(Duration::from_mins(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;

                match Self::api_call().await {
                    Ok((version, asset)) => {
                        info!(version = %version, "fetched latest release details from GitHub for app update");

                        let mut w = local_inner.write().await;
                        *w = Some(ClientUpdate {
                            version,
                            url: asset.browser_download_url,
                            required: false,
                        });
                    }
                    Err(e) => warn!(
                        error = ?e,
                        "could not fetch latest release details from GitHub for app updater"
                    ),
                }
            }
        });

        Self(inner)
    }

    pub async fn get(&self) -> Option<ClientUpdate> {
        let r = self.0.read().await;

        r.clone()
    }

    async fn api_call() -> Result<(SemVer, GhAsset)> {
        let client = reqwest::Client::builder()
            .user_agent("cloudpoint-server")
            .build()?;

        let response = client
            .get("https://api.github.com/repos/dwalker109/cloudpoint/releases/latest")
            .send()
            .await?;

        let data: GhResponse = response.json().await?;

        let asset = data
            .assets
            .into_iter()
            .find(|a| a.name == "cloudpoint.cia")
            .ok_or(anyhow!("no cia available"))?;

        Ok((data.tag_name, asset))
    }
}
