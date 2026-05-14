use crate::{
    ctr_fs::{self, CtrArchive},
    ctr_title::{
        SD_APP_TITLES, infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
    },
};
use anyhow::Result;
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
pub struct StateDb(#[serde[skip]] PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = root_path.as_ref().join("state.db");

        let state_db = match fs::read(&db_path) {
            Ok(buf) => {
                let mut state_db = postcard::from_bytes::<StateDb>(&buf)?;
                state_db.0 = db_path;
                state_db
            }
            Err(_) => Self(db_path, HashMap::new()),
        };

        Ok(state_db)
    }

    pub fn save(&mut self) -> Result<()> {
        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }

    pub fn discover_for_title_id(&mut self, title_id: u64, auto_enabled: bool) -> Result<()> {
        log::info!("processing {title_id:016X}");

        let mut process = |sync_item| -> Result<()> {
            if let Some(existing_state) = self.1.get_mut(&sync_item) {
                if existing_state.add_via_title_id(title_id) {
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

                let state = SyncState::new(
                    sync_item,
                    title_id,
                    &CtrArchive::smdh(sync_item)?,
                    auto_enabled,
                );

                self.1.insert(sync_item, state);
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

    pub fn discover_all(&mut self, auto_enabled: bool) -> Result<()> {
        log::info!("discovering all savedata and extdata");

        for title in SD_APP_TITLES.iter() {
            self.discover_for_title_id(title.title_id, auto_enabled)?;
        }

        Ok(())
    }

    pub fn toggle_for_title_id(&mut self, title_id: u64) -> Result<()> {
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
