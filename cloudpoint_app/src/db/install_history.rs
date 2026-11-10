use crate::{config::AppPath, title::get_installed_at_for_title};
use anyhow::Result;
use cloudpoint_lib::{db::Persist, sync::SyncItem};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

mod migrate;

#[derive(Serialize, Deserialize)]
pub struct InstallHistoryDb(HashMap<(u64, SyncItem), u64>);

impl Persist for InstallHistoryDb {
    fn path() -> PathBuf {
        AppPath::Db.join("install_history.db")
    }
}

impl InstallHistoryDb {
    pub fn new() -> Result<Self> {
        log::debug!("building install history db");

        let install_db = Self(HashMap::new());

        Ok(install_db)
    }

    pub fn check(&self, title_id: u64, sync_item: SyncItem) -> InstallStatus {
        let latest_mtime = &get_installed_at_for_title(title_id);
        let cached_mtime = self.0.get(&(title_id, sync_item));

        log::debug!("latest_mtime is {:?}", latest_mtime);
        log::debug!("cached_mtime is {:?}", cached_mtime);

        match (latest_mtime, cached_mtime) {
            (_, None) => InstallStatus::Updated,
            (latest, Some(cached)) if latest != cached => InstallStatus::Updated,
            (latest, Some(cached)) if latest == cached => InstallStatus::Unchanged,
            _ => unreachable!("install status cannot be unknown"),
        }
    }

    pub fn touch(&mut self, title_id: u64, sync_item: SyncItem) {
        self.0
            .insert((title_id, sync_item), get_installed_at_for_title(title_id));
    }
}

pub enum InstallStatus {
    Unchanged,
    Updated,
}
