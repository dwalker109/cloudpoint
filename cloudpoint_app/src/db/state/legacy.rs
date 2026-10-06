use crate::db::StateDb;
use chrono::{DateTime, Utc};
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

pub use v0::StateDbV0;

mod v0 {

    use super::*;

    #[derive(Deserialize)]
    pub struct StateDbV0(#[serde(skip)] PathBuf, HashMap<SyncItem, SyncStateV0>);

    #[derive(serde::Deserialize)]
    struct SyncStateV0 {
        pub sync_item: SyncItem,
        pub auto_enabled: bool,
        pub title_short: String,
        pub title_publisher: String,
        pub fs_safe_name: String,
        pub synced_fingerprint: Option<u128>,
        pub synced_at: Option<DateTime<Utc>>,
        pub via_title_ids: HashSet<u64>,
        pub via_user_key: Uuid,
    }

    impl From<StateDbV0> for StateDb {
        fn from(legacy: StateDbV0) -> Self {
            let StateDbV0(_path, items) = legacy;

            let updated_items = items
                .into_iter()
                .map(|(k, v)| {
                    let SyncStateV0 {
                        sync_item,
                        auto_enabled,
                        synced_fingerprint,
                        synced_at,
                        via_title_ids,
                        via_user_key,
                        title_short: _title_short,
                        title_publisher: _title_publisher,
                        fs_safe_name: _fs_safe_name,
                    } = v;

                    (
                        k,
                        SyncState {
                            sync_item,
                            auto_enabled,
                            synced_fingerprint,
                            synced_at,
                            via_title_ids,
                            via_user_key,
                        },
                    )
                })
                .collect();

            Self(updated_items)
        }
    }
}
