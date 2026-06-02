use crate::{
    app::{RefreshProgress, UiMsg},
    config::USER_KEY,
    ctr_fs::{self, CtrArchive},
    ctr_title::{
        SD_APP_TITLES, infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
    },
};
use anyhow::{Result, bail};
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

#[derive(Deserialize, Serialize)]
pub struct StateDb(#[serde[skip]] PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading all savedata and extdata from disk");

        let db_path = root_path.as_ref().join("state.db");

        if let Ok(buf) = fs::read(&db_path) {
            let mut state_db = postcard::from_bytes::<StateDb>(&buf)?;
            state_db.0 = db_path;

            Ok(state_db)
        } else {
            bail!("state db not found")
        }
    }

    pub fn new(root_path: impl AsRef<Path>, ui_tx: &Sender<UiMsg>) -> Result<Self> {
        log::debug!("building all savedata and extdata");

        let db_path = root_path.as_ref().join("state.db");

        let mut state_db = Self(db_path, HashMap::new());
        state_db.refresh_db(true, ui_tx)?;

        Ok(state_db)
    }

    pub fn save(&mut self) -> Result<()> {
        log::debug!("saving state db to disk");

        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }

    pub fn refresh_db(&mut self, auto_enabled: bool, ui_tx: &Sender<UiMsg>) -> Result<()> {
        log::debug!("refreshing all savedata and extdata");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());

        let total = SD_APP_TITLES.len();

        for (i, (&title_id, _)) in SD_APP_TITLES.iter().enumerate() {
            self.refresh_title(title_id, auto_enabled)?;
            refresh_progress
                .message("Refreshing sync items")
                .progress((i + 1) * 100 / total)
                .send();
        }

        let current_title_ids = SD_APP_TITLES.keys().copied().collect::<HashSet<_>>();

        for state in self.1.values_mut() {
            state.via_title_ids = state
                .via_title_ids
                .intersection(&current_title_ids)
                .copied()
                .collect();
        }

        self.1.retain(|_, s| !s.via_title_ids.is_empty());

        Ok(())
    }

    pub fn refresh_title(&mut self, title_id: u64, auto_enabled: bool) -> Result<()> {
        log::debug!("processing refresh for title {title_id:016X}");

        let mut process = |sync_item| -> Result<()> {
            if let Some(existing_state) = self.1.get_mut(&sync_item) {
                if existing_state.via_title_ids.insert(title_id) {
                    log::info!("updating {sync_item} reached via {title_id:016X}");

                    existing_state.auto_enabled = auto_enabled;
                } else {
                    log::info!(
                        "skipping {sync_item} discovered via {title_id:016X}, already tracked"
                    );
                }

                return Ok(());
            }

            if ctr_fs::CtrArchive::open(sync_item).is_ok() {
                log::info!("adding {sync_item} discovered via {title_id:016X}");

                self.1.insert(
                    sync_item,
                    SyncState::new(
                        sync_item,
                        title_id,
                        *USER_KEY,
                        &CtrArchive::smdh(sync_item)?,
                        auto_enabled,
                    ),
                );
            }

            Ok(())
        };

        let sync_item = SyncItem::Savedata(title_id);
        process(sync_item)?;

        if let Some(sync_item) = lookup_extdata_sync_item_for_title(title_id) {
            process(sync_item)?;
        } else if let Some(sync_item) = infer_extdata_sync_item_for_title(title_id) {
            process(sync_item)?;
        }

        Ok(())
    }

    pub fn toggle_title(&mut self, title_id: u64) -> Result<()> {
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
}

impl Drop for StateDb {
    fn drop(&mut self) {
        self.save()
            .expect("should be able to save state db on shutdown")
    }
}
