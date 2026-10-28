pub enum UpdaterMode {
    Enabled,
    Disabled,
}

pub mod client_app {
    use super::UpdaterMode;
    use anyhow::{Result, anyhow};
    use cloudpoint_lib::preflight::ClientUpdate;
    use reqwest::header;
    use semver::Version as SemVer;
    use serde::Deserialize;
    use std::{sync::Arc, time::Duration};
    use tokio::{sync::RwLock, task, time::interval};
    use tracing::{info, warn};

    #[derive(Clone)]
    pub struct Updater(Arc<RwLock<Option<ClientUpdate>>>);

    #[derive(Deserialize)]
    struct GhResponse {
        tag_name: SemVer,
        assets: Vec<GhAsset>,
    }

    #[derive(Deserialize)]
    struct GhAsset {
        browser_download_url: String,
        name: String,
        size: usize,
        digest: String,
    }

    impl Updater {
        pub fn new(mode: UpdaterMode) -> Self {
            let inner = Arc::new(RwLock::new(None));

            match mode {
                UpdaterMode::Enabled => {
                    info!("app update is enabled by runtime config");
                    task::spawn(update_task(Arc::clone(&inner)));
                }
                UpdaterMode::Disabled => {
                    info!("app update is disabled by runtime config");
                }
            }

            Self(inner)
        }

        pub async fn get(&self) -> Option<ClientUpdate> {
            let r = self.0.read().await;

            r.clone()
        }
    }

    async fn update_task(inner: Arc<RwLock<Option<ClientUpdate>>>) {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            "application/vnd.github+json".parse().unwrap(),
        );
        headers.insert("X-GitHub-Api-Version", "2026-03-10".parse().unwrap());

        let client = reqwest::Client::builder()
            .user_agent("cloudpoint-server")
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()
            .expect("should create reqwest client");

        let mut interval = interval(Duration::from_mins(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            match api_call(&client).await {
                Ok((version, asset)) => {
                    info!(version = %version, "fetched latest release details from GitHub for app update");

                    let mut w = inner.write().await;
                    *w = Some(ClientUpdate {
                        version,
                        url: asset.browser_download_url,
                        size: asset.size,
                        digest: asset.digest,
                        required: false,
                    });
                }
                Err(e) => warn!(
                    error = %e,
                    "could not fetch latest release details from GitHub for app updater"
                ),
            }
        }
    }

    async fn api_call(client: &reqwest::Client) -> Result<(SemVer, GhAsset)> {
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
