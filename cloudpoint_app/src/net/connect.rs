use crate::{app::UiMsg, config::USER_SETTINGS, net::HTTP_CLIENT};
use anyhow::{Result, bail};
use cloudpoint_lib::preflight::Connect;
use ctru::services::ac::Ac;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

pub fn run(shutdown_rx: &Receiver<()>, ui_tx: &Sender<UiMsg>) -> Result<Connect> {
    log::info!("connecting to server");

    let ac = Ac::new()?;
    let timeout = Instant::now();
    let delay = Duration::from_millis(100);
    let url = &format!("{}/api/v1/preflight", USER_SETTINGS.base_url);

    loop {
        if let Err(mpsc::TryRecvError::Disconnected) = shutdown_rx.try_recv() {
            bail!("aborting during connect due to app shutdown");
        }

        if timeout.elapsed() > Duration::from_secs(10) {
            ui_tx.send(UiMsg::ConnectDelayed).ok();
        }

        if let Err(e) = ac.wait_internet_connection() {
            log::debug!(
                "trying to await internet connection for {} seconds: {e}",
                timeout.elapsed().as_secs()
            );
            std::thread::sleep(delay);
            continue;
        }

        return match HTTP_CLIENT.get(url, &[]) {
            Ok(res) if res.status == 200 => {
                log::info!(
                    "connected to server, took {} seconds",
                    timeout.elapsed().as_secs()
                );

                Ok(postcard::from_bytes(&res.body).unwrap_or_default())
            }
            Ok(res) => {
                ui_tx
                    .send(UiMsg::ConnectFailed {
                        reason: String::from_utf8_lossy_owned(res.body),
                    })
                    .ok();

                bail!(
                    "connected to server but received status code {}, took {} seconds, cannot proceed",
                    res.status,
                    timeout.elapsed().as_secs()
                );
            }
            Err(e) => {
                log::warn!("failing to connect to server (will retry): {e}");
                ui_tx.send(UiMsg::ConnectDelayed).ok();
                std::thread::sleep(delay);
                continue;
            }
        };
    }
}
