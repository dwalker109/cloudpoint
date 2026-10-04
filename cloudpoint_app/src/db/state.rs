use crate::{
    app::{RefreshProgress, UiMsg},
    config::USER_KEY,
    db::{TitleDb, TitleDetails},
};
use anyhow::{Result, bail};
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

mod legacy;

const MAGIC: &[u8; 4] = b"CPDB";
const CURRENT_VERSION: u16 = 1;

#[derive(Deserialize, Serialize)]
pub struct StateDb(#[serde(skip)] PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading state db from disk");

        let db_path = root_path.as_ref().join("state.db");

        let Ok(buf) = fs::read(&db_path) else {
            bail!("state db not found")
        };

        let mut state_db = Self::decode(&buf)?;
        state_db.0 = db_path;

        Ok(state_db)
    }

    fn decode(buf: &[u8]) -> Result<Self> {
        let Some(rest) = buf.strip_prefix(MAGIC) else {
            log::info!("migrating unversioned (v0) state db");
            return Ok(postcard::from_bytes::<legacy::StateDbV0>(buf)?.into());
        };

        let Some((version, payload)) = rest.split_first_chunk::<2>() else {
            bail!("state db truncated")
        };

        match u16::from_le_bytes(*version) {
            1 => Ok(postcard::from_bytes(payload)?),
            _ => bail!("unsupported state db version"),
        }
    }

    pub fn new(
        root_path: impl AsRef<Path>,
        title_db: &TitleDb,
        ui_tx: &Sender<UiMsg>,
    ) -> Result<Self> {
        log::debug!("building new state db");

        let db_path = root_path.as_ref().join("state.db");

        let mut state_db = Self(db_path, HashMap::new());
        state_db.refresh(true, title_db, ui_tx);

        Ok(state_db)
    }

    pub fn refresh(&mut self, auto_enabled: bool, title_db: &TitleDb, ui_tx: &Sender<UiMsg>) {
        log::debug!("refreshing state db records");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());
        let total = title_db.titles_qty();

        for s in self.1.values_mut() {
            s.via_title_ids.clear();
        }

        for (i, title) in title_db.titles().enumerate() {
            self.process_sync_items_for_title(&title, auto_enabled);

            refresh_progress
                .message("Refreshing sync items")
                .progress((i + 1) * 100 / total)
                .send();
        }

        self.1.retain(|_, s| !s.via_title_ids.is_empty());
    }

    pub fn process_sync_items_for_title(&mut self, title: &TitleDetails, auto_enabled: bool) {
        log::debug!(
            "processing refresh for title {title_id:016X}",
            title_id = title.title_id
        );

        for sync_item in [title.savedata_sync_item, title.extdata_sync_item]
            .iter()
            .flatten()
        {
            match self.1.get_mut(sync_item) {
                Some(sync_state) => {
                    log::info!(
                        "updating {sync_item} reached via {title_id:016X}",
                        title_id = title.title_id
                    );
                    sync_state.via_title_ids.insert(title.title_id);
                }
                None => {
                    log::info!(
                        "adding {sync_item} discovered via {title_id:016X}",
                        title_id = title.title_id
                    );

                    self.1.insert(
                        *sync_item,
                        SyncState::new(*sync_item, title.title_id, *USER_KEY, auto_enabled),
                    );
                }
            }
        }
    }

    pub fn toggle_auto_sync_for_title(&mut self, title_id: u64) -> Result<()> {
        log::debug!("toggling auto sync enabled setting for title {title_id:016X}");

        let states = self
            .states_mut()
            .filter(|s| s.via_title_ids.contains(&title_id))
            .collect::<Vec<_>>();

        let toggle_to = if states.iter().all(|s| s.auto_enabled == true) {
            false
        } else if states.iter().all(|s| s.auto_enabled == false) {
            true
        } else {
            states
                .iter()
                .find_map(|s| {
                    matches!(s.sync_item, SyncItem::Extdata(..)).then_some(s.auto_enabled)
                })
                .or_else(|| states.first().map(|s| !s.auto_enabled))
                .unwrap_or_default()
        };

        for state in states {
            log::debug!(
                "state for {:?} was {}, toggling to {}",
                state.sync_item,
                state.auto_enabled,
                toggle_to
            );

            state.auto_enabled = toggle_to;
        }

        Ok(())
    }

    pub fn qty_total(&self) -> usize {
        self.1.len()
    }

    pub fn qty_auto(&self) -> usize {
        self.1.iter().filter(|s| s.1.auto_enabled).count()
    }

    pub fn state(&self, sync_item: &SyncItem) -> Option<&SyncState> {
        self.1.get(sync_item)
    }

    pub fn states(&self) -> impl Iterator<Item = &SyncState> {
        self.1.values()
    }

    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut SyncState> {
        self.1.values_mut()
    }

    pub fn states_hashmap(&self) -> HashMap<SyncItem, SyncState> {
        self.states().map(|s| (s.sync_item, s.clone())).collect()
    }

    pub fn commit(&mut self) -> Result<()> {
        log::debug!("saving state db to disk");

        let mut buf = Vec::with_capacity(MAGIC.len() + size_of::<u16>());
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&CURRENT_VERSION.to_le_bytes());
        buf.extend(postcard::to_allocvec(&self)?);

        fs::write(&self.0, buf)?;

        Ok(())
    }
}
