use crate::{
    app::{RefreshProgress, UiMsg},
    ctr_title::{SD_APP_TITLES, title_smdh},
};
use crate::{
    ctr_title::{
        infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
        lookup_savedata_sync_item_for_title,
    },
    db::StateDb,
};
use anyhow::{Result, bail};
use cloudpoint_lib::{
    ctr::{CtrSmdh, SmdhLanguage},
    sync::{SyncItem, SyncItemStatus, SyncState},
};
use itertools::Itertools;
use log::warn;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

#[derive(Serialize, Deserialize)]
pub struct TitleDb(#[serde[skip]] PathBuf, HashMap<u64, TitleDetails>);

impl TitleDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
        log::debug!("loading title db from disk");

        let db_path = root_path.as_ref().join("title.db");

        if let Ok(buf) = fs::read(&db_path) {
            let mut title_db = postcard::from_bytes::<TitleDb>(&buf)?;
            title_db.0 = db_path;

            Ok(title_db)
        } else {
            bail!("title db not found")
        }
    }

    pub fn new(
        root_path: impl AsRef<Path>,
        state_db: &StateDb,
        ui_tx: &Sender<UiMsg>,
    ) -> Result<Self> {
        log::debug!("building title db");

        let mut title_db = Self(root_path.as_ref().join("title.db"), HashMap::new());
        title_db.refresh(state_db, ui_tx)?;

        Ok(title_db)
    }

    pub fn prune_orphaned(&mut self) -> Result<()> {
        log::debug!("pruning orphaned title db records");

        let current_title_ids = SD_APP_TITLES.keys().copied().collect::<HashSet<_>>();
        self.1.retain(|k, _| current_title_ids.contains(k));

        Ok(())
    }

    pub fn refresh(&mut self, state_db: &StateDb, ui_tx: &Sender<UiMsg>) -> Result<()> {
        log::debug!("adding missing title db records");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());
        let total = SD_APP_TITLES.len();
        let states = state_db.states_hashmap();

        for (i, title_id) in SD_APP_TITLES.keys().enumerate() {
            if let Err(e) = try {
                self.add_or_replace_title(*title_id, &states)?;
            } {
                self.remove_title(*title_id)?;
                warn!("error processing title {title_id:016X}: {e}");
            };

            refresh_progress
                .message("Refreshing titles")
                .progress((i + 1) * 100 / total)
                .send();
        }

        Ok(())
    }

    fn add_or_replace_title(
        &mut self,
        title_id: u64,
        states: &HashMap<SyncItem, SyncState>,
    ) -> Result<()> {
        log::debug!("processing {title_id:016X}");

        let Some(title) = SD_APP_TITLES.get(&title_id) else {
            bail!("cannot find title {title_id:016X}");
        };

        let title_id = title.title_id;
        let product_code = &title.product_code;
        let smdh = title_smdh(title_id)?;

        let title = TitleDetails::new(title_id, &product_code, &smdh);

        if title.savedata_status(&states) != SyncItemStatus::Unavailable
            || title.extdata_status(&states) != SyncItemStatus::Unavailable
        {
            log::info!("added {title_id:016X}, has save or extdata");
            self.1.insert(title_id, title);
        } else {
            log::info!("ignored {title_id:016X}, has no save or extdata");
        }

        Ok(())
    }

    fn remove_title(&mut self, title_id: u64) -> Result<()> {
        self.1.remove(&title_id);

        Ok(())
    }

    pub fn title_mut(&mut self, title_id: u64) -> Option<&mut TitleDetails> {
        self.1.get_mut(&title_id)
    }

    pub fn total_titles(&self) -> usize {
        self.1.len()
    }

    pub fn titles_sorted_vec(&self) -> Vec<TitleDetails> {
        self.1
            .values()
            .sorted_by_key(|t| t.title_short.to_lowercase())
            .cloned()
            .collect()
    }

    fn save(&mut self) -> Result<()> {
        log::debug!("saving title db to disk");

        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
    }
}

impl Drop for TitleDb {
    fn drop(&mut self) {
        self.save()
            .expect("should be able to save title db on shutdown")
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TitleDetails {
    pub title_id: u64,
    pub product_code: String,
    pub title_short: String,
    pub title_publisher: String,
    pub savedata_sync_item: Option<SyncItem>,
    pub extdata_sync_item: Option<SyncItem>,
}

impl TitleDetails {
    pub fn new(title_id: u64, product_code: &str, smdh: &CtrSmdh) -> Self {
        let savedata_sync_item = lookup_savedata_sync_item_for_title(title_id);
        let extdata_sync_item = lookup_extdata_sync_item_for_title(title_id)
            .or_else(|| infer_extdata_sync_item_for_title(title_id));

        Self {
            title_id,
            product_code: product_code.to_string(),
            title_short: smdh.title_short(SmdhLanguage::English),
            title_publisher: smdh.title_publisher(SmdhLanguage::English),
            savedata_sync_item,
            extdata_sync_item,
        }
    }

    pub fn smdh(&self) -> Result<CtrSmdh> {
        Ok(title_smdh(self.title_id)?)
    }

    pub fn savedata_status(&self, states: &HashMap<SyncItem, SyncState>) -> SyncItemStatus {
        Self::sync_item_status(&self.savedata_sync_item, states)
    }

    pub fn extdata_status(&self, states: &HashMap<SyncItem, SyncState>) -> SyncItemStatus {
        Self::sync_item_status(&self.extdata_sync_item, states)
    }

    fn sync_item_status(
        sync_item: &Option<SyncItem>,
        states: &HashMap<SyncItem, SyncState>,
    ) -> SyncItemStatus {
        match sync_item {
            Some(s) => match states.get(s) {
                Some(s) => match s.auto_enabled {
                    true => SyncItemStatus::Enabled,
                    false => SyncItemStatus::Disabled,
                },
                None => SyncItemStatus::Unavailable,
            },
            None => SyncItemStatus::Unavailable,
        }
    }
}
