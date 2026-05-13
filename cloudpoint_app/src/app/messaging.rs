use std::{
    rc::Rc,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
        oneshot,
    },
};

use chrono::{DateTime, Utc};
use cloudpoint_lib::{http::CurlHttpClient, sync::SyncItem};

use crate::{
    config::AppPath,
    db::{StateDb, TitleDb},
    sync::{self, ConflictWinner},
};

pub enum TaskMsg {
    ReadySync,
    StartSyncAll,
    StartSyncTargeted(Vec<SyncItem>),
    Autodiscover,
    InvalidateTitleDb,
    BuildTitleDb,
}

pub enum UiMsg {
    SyncReady {
        total_states: usize,
    },
    SyncProgress {
        title_short: String,
        message: String,
    },
    SyncDone {
        result: String,
        message: String,
    },
    TitleDbInvalidated,
    TitleDbReady {
        title_db: Arc<TitleDb>,
    },
}

pub enum AlertMsg {
    ResolveConflict {
        title_label: String,
        title_remote_time: Option<DateTime<Utc>>,
        is_first_sync: bool,
        reply_tx: oneshot::Sender<ConflictWinner>,
    },
}

pub fn worker_thread(task_rx: Receiver<TaskMsg>, ui_tx: Sender<UiMsg>, alert_tx: Sender<AlertMsg>) {
    let mut state_db = StateDb::open(AppPath::Db).expect("state db should be accessible");
    let client = Rc::new(CurlHttpClient::new().expect("curl client should be available"));

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::ReadySync) => {
                let total_states = state_db.total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
            }
            Ok(TaskMsg::Autodiscover) => {
                state_db.append_discovered().ok();
                let total_states = state_db.total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
            }
            Ok(TaskMsg::StartSyncAll) => {
                match sync::run(
                    state_db.states_mut(),
                    ui_tx.clone(),
                    alert_tx.clone(),
                    &client,
                ) {
                    Ok(_) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Last sync completed at".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                    Err(err) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "An error occurred during sync".into(),
                            message: err.to_string(),
                        })
                        .ok(),
                };
            }
            Ok(TaskMsg::StartSyncTargeted(sync_items)) => {
                match sync::run(
                    state_db
                        .states_mut()
                        .filter(|s| sync_items.contains(&s.sync_item)),
                    ui_tx.clone(),
                    alert_tx.clone(),
                    &client,
                ) {
                    Ok(_) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Last sync completed at".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                    Err(err) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "An error occurred during sync".into(),
                            message: err.to_string(),
                        })
                        .ok(),
                };
            }
            Ok(TaskMsg::InvalidateTitleDb) => {
                ui_tx.send(UiMsg::TitleDbInvalidated).ok();
            }
            Ok(TaskMsg::BuildTitleDb) => {
                let title_db = TitleDb::build(&state_db).expect("should build runtime title db");
                ui_tx
                    .send(UiMsg::TitleDbReady {
                        title_db: Arc::new(title_db),
                    })
                    .ok();
            }
            Err(_) => return,
        }
    }
}
