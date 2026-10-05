use crate::{
    app::{RefreshProgress, UiMsg},
    config::{AppPath, USER_KEY},
    db::{TitleDb, TitleDetails},
};
use anyhow::{Context, Result, bail};
use cloudpoint_lib::sync::{SyncItem, SyncState};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{LazyLock, mpsc::Sender},
};

mod legacy;

const CURRENT_VERSION: u16 = 1;
static PATH: LazyLock<PathBuf> = LazyLock::new(|| AppPath::Db.join("state.db"));

#[derive(Deserialize, Serialize)]
pub struct StateDb(HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open() -> Result<Self> {
        log::debug!("loading state db from disk");

        Ok(Self::decode(
            &fs::read(&*PATH).context("state db not found")?,
        )?)
    }

    fn decode(buf: &[u8]) -> Result<Self> {
        match buf.split_first_chunk::<6>() {
            Some((head, rest)) if head[..4] == super::MAGIC => {
                match u16::from_le_bytes(head[4..].try_into()?) {
                    CURRENT_VERSION => Ok(postcard::from_bytes(rest)?),
                    0 => Ok(postcard::from_bytes::<legacy::StateDbV0>(buf)?.into()),
                    v => bail!("cannot decode state db version {v}"),
                }
            }
            _ => Ok(postcard::from_bytes::<legacy::StateDbV0>(buf)?.into()),
        }
    }

    pub fn commit(&mut self) -> Result<()> {
        log::debug!("saving state db to disk");

        super::write_to_disk(&*PATH, CURRENT_VERSION, &self)
    }

    pub fn new(title_db: &TitleDb, ui_tx: &Sender<UiMsg>) -> Result<Self> {
        log::debug!("building new state db");

        let mut state_db = Self(HashMap::new());
        state_db.refresh(true, title_db, ui_tx);

        Ok(state_db)
    }

    pub fn refresh(&mut self, auto_enabled: bool, title_db: &TitleDb, ui_tx: &Sender<UiMsg>) {
        log::debug!("refreshing state db records");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());
        let total = title_db.titles_qty();

        for s in self.0.values_mut() {
            s.via_title_ids.clear();
        }

        for (i, title) in title_db.titles().enumerate() {
            self.process_sync_items_for_title(&title, auto_enabled);

            refresh_progress
                .message("Refreshing sync items")
                .progress((i + 1) * 100 / total)
                .send();
        }

        self.0.retain(|_, s| !s.via_title_ids.is_empty());
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
            match self.0.get_mut(sync_item) {
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

                    self.0.insert(
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
            .0
            .values_mut()
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
        self.0.len()
    }

    pub fn qty_auto(&self) -> usize {
        self.0.iter().filter(|s| s.1.auto_enabled).count()
    }

    pub fn state(&self, sync_item: &SyncItem) -> Option<&SyncState> {
        self.0.get(sync_item)
    }

    pub fn states(&self) -> impl Iterator<Item = &SyncState> {
        self.0.values()
    }

    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut SyncState> {
        self.0.values_mut()
    }

    pub fn states_hashmap(&self) -> HashMap<SyncItem, SyncState> {
        self.0.values().map(|s| (s.sync_item, s.clone())).collect()
    }
}
