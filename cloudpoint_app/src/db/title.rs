use crate::ctr_title::{
    infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
    lookup_savedata_sync_item_for_title,
};
use crate::{
    app::{RefreshProgress, UiMsg},
    ctr_title::{SD_APP_TITLES, lookup_gba_sync_item_for_title, title_smdh},
};
use anyhow::{Result, bail};
use cloudpoint_lib::utils::ellipsis;
use cloudpoint_lib::{
    ctr::{CtrSmdh, SmdhLanguage},
    sync::{SyncItem, SyncItemStatus, SyncState},
};
use itertools::Itertools;
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

    pub fn new(root_path: impl AsRef<Path>, ui_tx: &Sender<UiMsg>) -> Result<Self> {
        log::debug!("building title db");

        let mut title_db = Self(root_path.as_ref().join("title.db"), HashMap::new());
        title_db.refresh(ui_tx)?;

        Ok(title_db)
    }

    pub fn refresh(&mut self, ui_tx: &Sender<UiMsg>) -> Result<()> {
        log::debug!("refreshing title db records");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());
        let total = SD_APP_TITLES.len();

        for (i, title_id) in SD_APP_TITLES.keys().enumerate() {
            match self.handle_title(*title_id) {
                Ok(_) => log::info!("processed title {title_id:016X}"),
                Err(e) => match self.1.remove(&title_id) {
                    Some(_) => {
                        log::warn!("could not process title {title_id:016X}, was removed: {e}")
                    }
                    None => {
                        log::warn!("could not process title {title_id:016X}, was not added: {e}")
                    }
                },
            }

            refresh_progress
                .message("Refreshing titles")
                .progress((i + 1) * 100 / total)
                .send();
        }

        self.prune_orphaned();

        Ok(())
    }

    pub fn prune_orphaned(&mut self) {
        log::debug!("pruning orphaned title db records");

        let current_title_ids = SD_APP_TITLES.keys().copied().collect::<HashSet<_>>();
        self.1.retain(|k, _| current_title_ids.contains(k));
    }

    fn handle_title(&mut self, title_id: u64) -> Result<()> {
        log::debug!("processing {title_id:016X}");

        let (title_id, product_code) = match SD_APP_TITLES.get(&title_id) {
            Some(title) => (title.title_id, title.product_code.clone()),
            None => bail!("cannot find title {title_id:016X}"),
        };

        let savedata_sync_item = lookup_savedata_sync_item_for_title(title_id)
            .or_else(|| lookup_gba_sync_item_for_title(title_id));

        let extdata_sync_item = lookup_extdata_sync_item_for_title(title_id)
            .or_else(|| infer_extdata_sync_item_for_title(title_id));

        if let (None, None) = (savedata_sync_item, extdata_sync_item) {
            bail!("no savedata/gba savedata/extdata for title {title_id:016X}")
        };

        match self.1.get_mut(&title_id) {
            Some(title) => {
                title.savedata_sync_item = savedata_sync_item;
                title.extdata_sync_item = extdata_sync_item;
            }
            None => {
                let smdh = title_smdh(title_id)?;

                let title = TitleDetails {
                    title_id,
                    product_code,
                    title_short: smdh.title_short(SmdhLanguage::English),
                    title_publisher: smdh.title_publisher(SmdhLanguage::English),
                    savedata_sync_item,
                    extdata_sync_item,
                };

                self.1.insert(title_id, title);
            }
        }

        Ok(())
    }

    pub fn title_mut(&mut self, title_id: u64) -> Option<&mut TitleDetails> {
        self.1.get_mut(&title_id)
    }

    pub fn titles(&self) -> impl Iterator<Item = &TitleDetails> {
        self.1.values()
    }

    pub fn titles_qty(&self) -> usize {
        self.1.len()
    }

    pub fn titles_sorted_vec(&self) -> Vec<TitleDetails> {
        self.1
            .values()
            .sorted_by_key(|t| t.title_short.to_lowercase())
            .cloned()
            .collect()
    }

    pub fn sync_state_label(&self, sync_state: &SyncState) -> String {
        let (mut t, mut p) = (Vec::new(), Vec::new());

        for title in self
            .1
            .values()
            .filter(|t| sync_state.via_title_ids.contains(&t.title_id))
        {
            t.push(title.title_short.clone());
            p.push(title.title_publisher.clone());
        }

        ellipsis(&format!("{} ({})", t.join("/"), p.join("/")), 35)
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
        let savedata_sync_item = lookup_savedata_sync_item_for_title(title_id)
            .or_else(|| lookup_gba_sync_item_for_title(title_id));
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
