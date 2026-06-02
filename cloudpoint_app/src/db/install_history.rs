use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::ctr_title::get_installed_at_for_title;

#[derive(Deserialize, Serialize)]
pub struct InstallHistoryDb(#[serde[skip]] PathBuf, HashMap<u64, u64>);

impl InstallHistoryDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading install history db from disk");

        let db_path = root_path.as_ref().join("install_history.db");

        if let Ok(buf) = fs::read(&db_path) {
            let mut install_db = postcard::from_bytes::<InstallHistoryDb>(&buf)?;
            install_db.0 = db_path;

            Ok(install_db)
        } else {
            bail!("install history db not found")
        }
    }

    pub fn new(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("building install history db");

        let db_path = root_path.as_ref().join("install_history.db");
        let install_db = Self(db_path, HashMap::new());

        Ok(install_db)
    }

    pub fn check(&self, title_id: u64) -> InstallStatus {
        let latest_mtime = &get_installed_at_for_title(title_id);
        let cached_mtime = self.1.get(&title_id);

        log::debug!("latest_mtime is {:?}", latest_mtime);
        log::debug!("cached_mtime is {:?}", cached_mtime);

        match (latest_mtime, cached_mtime) {
            (Ok(_), None) => InstallStatus::Updated,
            (Ok(latest), Some(cached)) if latest != cached => InstallStatus::Updated,
            (Ok(latest), Some(cached)) if latest == cached => InstallStatus::Unchanged,
            _ => InstallStatus::Unknown,
        }
    }

    pub fn touch(&mut self, title_id: u64) {
        self.1.insert(
            title_id,
            get_installed_at_for_title(title_id)
                .expect("install mtime should be available for title"),
        );
    }

    fn save(&mut self) -> Result<()> {
        log::debug!("saving install history db to disk");

        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }
}

impl Drop for InstallHistoryDb {
    fn drop(&mut self) {
        self.save()
            .expect("should be able to save install history db on shutdown")
    }
}

pub enum InstallStatus {
    Unknown,
    Unchanged,
    Updated,
}
