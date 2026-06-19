use crate::{
    db::TitleDetails,
    link::{LinkState, SharePermission},
};
use chrono::{DateTime, Utc};
use std::sync::{mpsc::Sender, oneshot};

pub enum TaskMsg {
    SyncAuto,
    SyncTargeted(u64),
    Toggle(u64),
    Refresh,
    LinkHost,
    LinkClient,
}

pub enum OpenModalMsg {
    Connect,
    Refresh,
    ResolveConflict {
        title_label: String,
        title_local_time: Option<DateTime<Utc>>,
        title_remote_time: Option<DateTime<Utc>>,
        reply_tx: oneshot::Sender<ConflictWinner>,
    },
    Error {
        label: String,
        message: String,
    },
    LinkHost {
        quit_tx: Sender<()>,
    },
    LinkClient {
        fc: u64,
        quit_tx: Sender<()>,
    },
}

pub enum UiMsg {
    ConnectDelayed,
    ConnectDone,
    ConnectFailed {
        reason: String,
    },
    SyncProgress {
        label: String,
        message: String,
        progress: usize,
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
    LinkHostConfirm {
        friend_code: String,
        reply_tx: Sender<SharePermission>,
        state: LinkState,
    },
    LinkClientConfirm {
        new_user_key: String,
        reply_tx: Sender<SharePermission>,
        state: LinkState,
    },
    LinkUpdate {
        state: LinkState,
    },
}

pub enum ConflictWinner {
    Local,
    Remote,
    Undecided,
}

pub struct SyncProgress {
    sender: Sender<UiMsg>,
    label: String,
    message: String,
    progress: usize,
}

impl SyncProgress {
    pub fn new(sender: Sender<UiMsg>) -> Self {
        Self {
            sender,
            label: String::with_capacity(64),
            message: String::with_capacity(64),
            progress: 0,
        }
    }

    pub fn label(&mut self, label: &str) -> &mut Self {
        self.label = label.into();
        self
    }

    pub fn message(&mut self, message: &str) -> &mut Self {
        self.message = message.into();
        self
    }

    pub fn progress(&mut self, progress: usize) -> &mut Self {
        self.progress = progress;
        self
    }

    pub fn send(&self) {
        self.sender
            .send(UiMsg::SyncProgress {
                label: self.label.clone(),
                message: self.message.clone(),
                progress: self.progress,
            })
            .ok();
    }
}

impl Drop for SyncProgress {
    fn drop(&mut self) {
        self.progress = 100;
        self.send();
    }
}

pub struct RefreshProgress {
    sender: Sender<UiMsg>,
    message: String,
    progress: usize,
}

impl RefreshProgress {
    pub fn new(sender: Sender<UiMsg>) -> Self {
        Self {
            sender,
            message: String::with_capacity(64),
            progress: 0,
        }
    }

    pub fn message(&mut self, message: &str) -> &mut Self {
        self.message = message.into();
        self
    }

    pub fn progress(&mut self, progress: usize) -> &mut Self {
        self.progress = progress;
        self
    }

    pub fn send(&self) {
        self.sender
            .send(UiMsg::RefreshProgress {
                message: self.message.clone(),
                progress: self.progress,
            })
            .ok();
    }
}

impl Drop for RefreshProgress {
    fn drop(&mut self) {
        self.progress = 100;
        self.send();
    }
}
