use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::ctr_title::get_installed_at_for_title;

#[derive(Deserialize, Serialize)]
pub struct InstallDb(#[serde[skip]] PathBuf, HashMap<u64, u64>);

impl InstallDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading install db from disk");

        let db_path = root_path.as_ref().join("install.db");

        if let Ok(buf) = fs::read(&db_path) {
            let mut install_db = postcard::from_bytes::<InstallDb>(&buf)?;
            install_db.0 = db_path;

            Ok(install_db)
        } else {
            bail!("install db not found")
        }
    }

    pub fn new(root_path: impl AsRef<Path>) -> Self {
        log::debug!("building install db");

        let db_path = root_path.as_ref().join("install.db");
        let install_db = Self(db_path, HashMap::new());

        install_db
    }

    pub fn save(&mut self) -> Result<()> {
        log::debug!("saving install db to disk");

        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }

    pub fn check_install(&self, title_id: u64) -> InstallStatus {
        let latest_mtime = &get_installed_at_for_title(title_id);
        let cached = self.1.get(&title_id);

        match (latest_mtime, cached) {
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
}

impl Drop for InstallDb {
    fn drop(&mut self) {
        self.save()
            .expect("should be able to save install db on shutdown")
    }
}

pub enum InstallStatus {
    Unknown,
    Unchanged,
    Updated,
}
