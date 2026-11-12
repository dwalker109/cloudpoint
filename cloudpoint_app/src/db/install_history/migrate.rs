use super::InstallHistoryDb as InstallHistoryDbV2;
use cloudpoint_lib::{
    db::{Migrate, MigrateRoot},
    sync::SyncItem,
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

impl Migrate for InstallHistoryDbV2 {
    const VERSION_ID: u16 = 2;

    type Previous = InstallHistoryDbV1;

    fn migrate(prev: Self::Previous) -> anyhow::Result<Self> {
        let InstallHistoryDbV1(items) = prev;

        let updated_items = items
            .into_iter()
            .map(|(k, v)| match v {
                0 => (k, None),
                n => (k, Some(n)),
            })
            .collect();

        Ok(Self(updated_items))
    }
}

#[derive(Deserialize)]
pub struct InstallHistoryDbV1(HashMap<(u64, SyncItem), u64>);

impl Migrate for InstallHistoryDbV1 {
    const VERSION_ID: u16 = 1;

    type Previous = InstallHistoryDbV0;

    fn migrate(prev: Self::Previous) -> anyhow::Result<Self> {
        let InstallHistoryDbV0(_path, items) = prev;

        Ok(Self(items))
    }
}

#[derive(Deserialize)]
pub struct InstallHistoryDbV0(#[serde(skip)] PathBuf, HashMap<(u64, SyncItem), u64>);

impl Migrate for InstallHistoryDbV0 {
    const VERSION_ID: u16 = 0;

    type Previous = MigrateRoot;

    fn migrate(_prev: Self::Previous) -> anyhow::Result<Self> {
        unreachable!()
    }
}
