use crate::{
    app::{AlertMsg, TaskMsg, UiMsg, worker_thread},
    config::AppPath,
};
use anyhow::{Context, Result};
use std::{
    fs,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender},
    },
    thread::JoinHandle,
};

pub fn sdmc() -> Result<()> {
    let paths = [AppPath::Base, AppPath::Db, AppPath::Log];
    for p in paths {
        fs::create_dir_all(&p).with_context(|| {
            format!("fatal: failed to create directory {}", p.as_ref().display())
        })?;
    }

    log::debug!("Created paths");

    Ok(())
}

pub fn start_worker(
    task_rx: Receiver<TaskMsg>,
    ui_tx: Sender<UiMsg>,
    alert_tx: Sender<AlertMsg>,
) -> Result<JoinHandle<()>> {
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || worker_thread(task_rx, ui_tx, alert_tx))?;

    Ok(handle)
}
