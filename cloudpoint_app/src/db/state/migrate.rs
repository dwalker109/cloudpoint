use crate::db::StateDb as StateDbV2;
use chrono::{DateTime, Utc};
use cloudpoint_lib::{
    db::{Migrate, MigrateRoot},
    sync::{SyncItem, SyncState as SyncStateV2},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use uuid::Uuid;

impl Migrate for StateDbV2 {
    const VERSION_ID: u16 = 2;

    type Previous = StateDbV1;

    fn migrate(prev: Self::Previous) -> anyhow::Result<Self> {
        let StateDbV1(items) = prev;

        let updated_items = items
            .into_iter()
            .map(|(k, v)| {
                let SyncStateV1 {
                    sync_item,
                    auto_enabled,
                    synced_fingerprint,
                    synced_at,
                    via_title_ids,
                    via_user_key,
                } = v;

                (
                    k,
                    SyncStateV2 {
                        sync_item,
                        auto_enabled,
                        synced_fingerprint,
                        synced_at: synced_at.and_then(|dt| Some(dt.naive_utc())),
                        via_title_ids,
                        via_user_key,
                    },
                )
            })
            .collect();

        Ok(Self(updated_items))
    }
}

#[derive(Deserialize)]
pub struct StateDbV1(HashMap<SyncItem, SyncStateV1>);

#[derive(serde::Deserialize)]
struct SyncStateV1 {
    sync_item: SyncItem,
    auto_enabled: bool,
    synced_fingerprint: Option<u128>,
    synced_at: Option<DateTime<Utc>>,
    via_title_ids: HashSet<u64>,
    via_user_key: Uuid,
}

impl Migrate for StateDbV1 {
    const VERSION_ID: u16 = 1;

    type Previous = StateDbV0;

    fn migrate(prev: Self::Previous) -> anyhow::Result<Self> {
        let StateDbV0(_path, items) = prev;

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
                    SyncStateV1 {
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

        Ok(Self(updated_items))
    }
}

#[derive(Deserialize)]
pub struct StateDbV0(#[serde(skip)] PathBuf, HashMap<SyncItem, SyncStateV0>);

#[derive(serde::Deserialize)]
struct SyncStateV0 {
    sync_item: SyncItem,
    auto_enabled: bool,
    title_short: String,
    title_publisher: String,
    fs_safe_name: String,
    synced_fingerprint: Option<u128>,
    synced_at: Option<DateTime<Utc>>,
    via_title_ids: HashSet<u64>,
    via_user_key: Uuid,
}

impl Migrate for StateDbV0 {
    const VERSION_ID: u16 = 0;

    type Previous = MigrateRoot;

    fn migrate(_prev: Self::Previous) -> anyhow::Result<Self> {
        unreachable!()
    }
}
