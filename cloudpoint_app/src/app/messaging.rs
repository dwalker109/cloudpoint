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
    SyncDone,
    TitleDbReady {
        title_db: TitleDb,
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
                let _res = sync::run(Arc::clone(&state_db), ui_tx.clone(), alert_tx.clone());
                ui_tx.send(UiMsg::SyncDone).ok();
            }
            Ok(TaskMsg::BuildTitleDb) => {
                let title_db =
                    TitleDb::build(&state_db.read().expect("should get read lock for state db"))
                        .expect("should build runtime title db");
                ui_tx.send(UiMsg::TitleDbReady { title_db }).ok();
            }
            Err(_) => return,
        }
    }
}
