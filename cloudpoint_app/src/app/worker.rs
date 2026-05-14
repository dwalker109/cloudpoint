use std::{
    rc::Rc,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
};

use chrono::{DateTime, Utc};
use cloudpoint_lib::http::CurlHttpClient;

use crate::{
    config::AppPath,
    db::{StateDb, TitleDb, TitleDetails},
    sync::{self, ConflictWinner},
};

pub enum TaskMsg {
    // SyncReady,
    // TitleDbReady,
    SyncAuto,
    SyncTargeted(u64),
    Toggle(u64),
    Refresh,
}

pub enum UiMsg {
    // SyncReady { qty: usize },
    SyncProgress {
        title_lbl: String,
        message: String,
    },
    SyncDone {
        result: String,
        message: String,
    },
    RefreshProgress {
        message: String,
        progress: usize,
    },
    RefreshDone {
        qty_sync_states: usize,
        titles: Vec<TitleDetails>,
    },
}

pub enum ModalMsg {
    ResolveConflict {
        title_label: String,
        title_remote_time: Option<DateTime<Utc>>,
        is_first_sync: bool,
        reply_tx: oneshot::Sender<ConflictWinner>,
    },
    Refresh,
}

pub fn worker_thread(task_rx: Receiver<TaskMsg>, ui_tx: Sender<UiMsg>, modal_tx: Sender<ModalMsg>) {
    let mut state_db = StateDb::open(AppPath::Db)
        .or_else(|_| StateDb::new(AppPath::Db))
        .expect("state db should be accessible");
    let mut title_db = TitleDb::open(AppPath::Db)
        .or_else(|_| TitleDb::new(AppPath::Db, &state_db))
        .expect("title db should be accessible");

    let client = Rc::new(CurlHttpClient::new().expect("curl client should be available"));

    loop {
        match task_rx.recv() {
            Ok(TaskMsg::Refresh) => {
                modal_tx.send(ModalMsg::Refresh).ok();
                ui_tx
                    .send(UiMsg::RefreshProgress {
                        message: "Refreshing sync items".into(),
                        progress: 0,
                    })
                    .ok();
                state_db.refresh(true).ok();
                ui_tx
                    .send(UiMsg::RefreshProgress {
                        message: "Refreshing titles".into(),
                        progress: 50,
                    })
                    .ok();
                title_db.refresh(&state_db).ok();
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::Toggle(title_id)) => {
                state_db.refresh_for_title_id(title_id, false).ok();
                state_db.toggle_for_title_id(title_id).ok();
                title_db.refresh_links(title_id, &state_db).ok();
                ui_tx
                    .send(UiMsg::RefreshDone {
                        qty_sync_states: state_db.qty_auto(),
                        titles: title_db.titles_sorted_vec(),
                    })
                    .ok();
            }
            Ok(TaskMsg::SyncAuto) => {
                match sync::run(
                    state_db.states_mut().filter(|s| s.auto_enabled),
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                ) {
                    Ok(_) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Sync completed at ".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                    Err(_err) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Sync failed at".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                };
            }
            Ok(TaskMsg::SyncTargeted(title_id)) => {
                state_db.refresh_for_title_id(title_id, false).ok();
                title_db.refresh_links(title_id, &state_db).ok();
                match sync::run(
                    state_db
                        .states_mut()
                        .filter(|s| s.via_title_ids.contains(&title_id)),
                    ui_tx.clone(),
                    modal_tx.clone(),
                    &client,
                ) {
                    Ok(_) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Sync completed at ".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                    Err(_err) => ui_tx
                        .send(UiMsg::SyncDone {
                            result: "Sync failed at".into(),
                            message: chrono::Utc::now().to_rfc2822(),
                        })
                        .ok(),
                };
            }
            Err(_) => return,
        }
    }
}
