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

#[derive(Deserialize, Serialize)]
pub struct StateDb(#[serde[skip]] PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading state db from disk");

        let db_path = root_path.as_ref().join("state.db");

        if let Ok(buf) = fs::read(&db_path) {
            let mut state_db = postcard::from_bytes::<StateDb>(&buf)?;
            state_db.0 = db_path;

            Ok(state_db)
        } else {
            bail!("state db not found")
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
        state_db.refresh(true, title_db, ui_tx)?;

        Ok(state_db)
    }

    pub fn refresh(
        &mut self,
        auto_enabled: bool,
        title_db: &TitleDb,
        ui_tx: &Sender<UiMsg>,
    ) -> Result<()> {
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

        self.prune_orphaned();

        Ok(())
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

    pub fn prune_orphaned(&mut self) {
        log::debug!("pruning orphaned state db records");
        self.1.retain(|_, s| !s.via_title_ids.is_empty());
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

    fn save(&mut self) -> Result<()> {
        log::debug!("saving state db to disk");

        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }
}

impl Drop for StateDb {
    fn drop(&mut self) {
        self.save()
            .expect("should be able to save state db on shutdown")
    }
}
