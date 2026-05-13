use std::sync::{
    Arc, RwLock,
    mpsc::{Receiver, Sender},
    oneshot,
};

use chrono::{DateTime, Utc};

use crate::{
    db::{StateDb, TitleDb},
    sync::{self, ConflictWinner},
};

pub enum TaskMsg {
    ReadySync,
    StartSync,
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

pub fn handle_worker(
    state_db: Arc<RwLock<StateDb>>,
    task_rx: Receiver<TaskMsg>,
    ui_tx: Sender<UiMsg>,
    alert_tx: Sender<AlertMsg>,
) {
    loop {
        match task_rx.recv() {
            Ok(TaskMsg::ReadySync) => {
                let total_states = state_db
                    .read()
                    .expect("should get read lock for state db")
                    .total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
            }
            Ok(TaskMsg::Autodiscover) => {
                state_db
                    .write()
                    .expect("should get write lock for state db")
                    .append_discovered()
                    .ok();
                let total_states = state_db
                    .read()
                    .expect("should get read lock for state db")
                    .total_states();
                ui_tx.send(UiMsg::SyncReady { total_states }).ok();
            }
            Ok(TaskMsg::StartSync) => {
                match sync::run(Arc::clone(&state_db), ui_tx.clone(), alert_tx.clone()) {
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
                let title_db =
                    TitleDb::build(Arc::clone(&state_db)).expect("should build runtime title db");
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
