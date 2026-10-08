use crate::{config::AppPath, title::get_installed_at_for_title};
use anyhow::{Context, Result, bail};
use cloudpoint_lib::sync::SyncItem;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, sync::LazyLock};

mod legacy;

const CURRENT_VERSION: u16 = 1;
static PATH: LazyLock<PathBuf> = LazyLock::new(|| AppPath::Db.join("install_history.db"));

#[derive(Deserialize, Serialize)]
pub struct InstallHistoryDb(HashMap<(u64, SyncItem), u64>);

impl InstallHistoryDb {
    pub fn open() -> Result<Self> {
        log::debug!("loading install history db from disk");

        Ok(Self::decode(
            &fs::read(&*PATH).context("install history db not found")?,
        )?)
    }

    fn decode(buf: &[u8]) -> Result<Self> {
        let (ver, data) = super::decode_parts(buf)?;

        Ok(match ver {
            CURRENT_VERSION => postcard::from_bytes(data)?,
            0 => postcard::from_bytes::<legacy::InstallHistoryDbV0>(data)?.into(),
            v => bail!("cannot decode install history db version {v}"),
        })
    }

    pub fn commit(&mut self) -> Result<()> {
        log::debug!("saving install history db to disk");

        super::write_to_disk(&*PATH, CURRENT_VERSION, &self)
    }

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
