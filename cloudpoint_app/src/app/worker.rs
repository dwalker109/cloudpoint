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
    SyncReady,
    SyncAll,
    SyncTargeted(Vec<SyncItem>),
    DiscoverAll,
    DiscoverTargeted(u64),
    TitleDbBuild,
    TitleDbInvalidate,
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
            Ok(TaskMsg::SyncReady) => {
                let total_states = state_db.total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
            }
            Ok(TaskMsg::DiscoverAll) => {
                state_db.discover_all().ok();
                let total_states = state_db.total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
                ui_tx.send(UiMsg::TitleDbInvalidated).ok();
            }
            Ok(TaskMsg::DiscoverTargeted(title_id)) => {
                state_db.discover_for_title_id(title_id).ok();
                let total_states = state_db.total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
                ui_tx.send(UiMsg::TitleDbInvalidated).ok();
            }
            Ok(TaskMsg::SyncAll) => {
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
            Ok(TaskMsg::SyncTargeted(sync_items)) => {
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
            Ok(TaskMsg::TitleDbInvalidate) => {
                ui_tx.send(UiMsg::TitleDbInvalidated).ok();
            }
            Ok(TaskMsg::TitleDbBuild) => {
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
