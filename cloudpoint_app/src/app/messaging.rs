use std::sync::{
    Arc, RwLock,
    mpsc::{Receiver, Sender},
    oneshot,
};

use crate::{
    db::StateDb,
    sync::{self, ConflictWinner},
};

pub enum TaskMsg {
    ReadySync,
    StartSync,
    ReadyDiscover,
    StartDiscover,
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
    DiscoverReady {
        total_states: usize,
    },
    DiscoverDone {
        total_states: usize,
    },
}

pub enum AlertMsg {
    ResolveConflict {
        title_short: String,
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
            Ok(TaskMsg::StartSync) => {
                let _res = sync::run(Arc::clone(&state_db), ui_tx.clone(), alert_tx.clone());
                ui_tx.send(UiMsg::SyncDone).ok();
            }
            Ok(TaskMsg::ReadyDiscover) => {
                let total_states = state_db
                    .read()
                    .expect("should get read lock for state db")
                    .total_states();
                ui_tx.send(UiMsg::DiscoverReady { total_states }).ok();
            }
            Ok(TaskMsg::StartDiscover) => {
                state_db
                    .write()
                    .expect("should get write lock for state db")
                    .append_discovered()
                    .ok();
                let total_states = state_db
                    .read()
                    .expect("should get read lock for state db")
                    .total_states();
                ui_tx.send(UiMsg::DiscoverDone { total_states }).ok();
            }
            Err(_) => return,
        }
    }
}
