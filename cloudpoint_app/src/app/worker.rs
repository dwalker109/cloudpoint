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
    db::{StateDb, TitleDb, TitleDetails},
    sync::{self, ConflictWinner},
};

pub enum TaskMsg {
    SyncReady,
    SyncAllAuto,
    SyncTargeted(Vec<SyncItem>),
    DiscoverAll,
    DiscoverTargeted(u64),
    ToggleTargeted(u64),
    TitleDbReady,
    TitleDbInvalidate,
}

pub enum UiMsg {
    SyncReady { qty: usize },
    SyncProgress { title_lbl: String, message: String },
    SyncDone { result: String, message: String },
    TitleDbInvalidated,
    TitleDbReady { titles: Vec<TitleDetails> },
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
    let mut title_db = TitleDb::open(AppPath::Db)
        .or_else(|_| TitleDb::new(AppPath::Db, &state_db))
        .expect("title db should be accessible");

    let client = Rc::new(CurlHttpClient::new().expect("curl client should be available"));

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::DiscoverAll) => {
                state_db.discover_all(true).ok();
                ui_tx
                    .send(UiMsg::SyncReady {
                        qty: state_db.qty_auto(),
                    })
                    .ok();
                ui_tx.send(UiMsg::TitleDbInvalidated).ok();
            }
            Ok(TaskMsg::DiscoverTargeted(title_id)) => {
                state_db.discover_for_title_id(title_id, false).ok();
                title_db.refresh_cascade(title_id, &state_db).ok();
                ui_tx
                    .send(UiMsg::SyncReady {
                        qty: state_db.qty_auto(),
                    })
                    .ok();
                ui_tx
                    .send(UiMsg::TitleDbReady {
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::ToggleTargeted(title_id)) => {
                state_db.toggle_for_title_id(title_id).ok();
                title_db.refresh_cascade(title_id, &state_db).ok();
                ui_tx
                    .send(UiMsg::SyncReady {
                        qty: state_db.qty_auto(),
                    })
                    .ok();
                ui_tx
                    .send(UiMsg::TitleDbReady {
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::SyncReady) => {
                ui_tx
                    .send(UiMsg::SyncReady {
                        qty: state_db.qty_auto(),
                    })
                    .ok();
            }
            Ok(TaskMsg::SyncAllAuto) => {
                match sync::run(
                    state_db.states_mut().filter(|s| s.auto_enabled),
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
            Ok(TaskMsg::TitleDbReady) => {
                ui_tx
                    .send(UiMsg::TitleDbReady {
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Err(_) => return,
        }
    }
}
