use chrono::{DateTime, Utc};
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use uuid::Uuid;

use crate::db::StateDb;

#[derive(Deserialize)]
pub(super) struct StateDbV0(#[serde(skip)] PathBuf, HashMap<SyncItem, SyncStateV0>);

#[derive(serde::Deserialize)]
pub struct SyncStateV0 {
    pub sync_item: SyncItem,
    pub auto_enabled: bool,
    pub _title_short: String,
    pub _title_publisher: String,
    pub _fs_safe_name: String,
    pub synced_fingerprint: Option<u128>,
    pub synced_at: Option<DateTime<Utc>>,
    pub via_title_ids: HashSet<u64>,
    pub via_user_key: Uuid,
}

impl From<StateDbV0> for StateDb {
    fn from(v0: StateDbV0) -> Self {
        let mut state_db = Self(v0.0, HashMap::new());

        for v in v0.1.values() {
            state_db.1.insert(
                v.sync_item,
                SyncState {
                    sync_item: v.sync_item,
                    auto_enabled: v.auto_enabled,
                    synced_fingerprint: v.synced_fingerprint,
                    synced_at: v.synced_at,
                    via_title_ids: v.via_title_ids.clone(),
                    via_user_key: v.via_user_key,
                },
            );
        }

        state_db
    }
}
