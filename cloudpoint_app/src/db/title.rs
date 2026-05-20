use crate::{
    app::{RefreshProgress, UiMsg},
    ctr_title::{self, SD_APP_TITLES},
};
use anyhow::{Result, bail};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
};

use cloudpoint_lib::{
    ctr::{CtrSmdh, SmdhLanguage},
    sync::SyncItem,
};

use crate::{
    ctr_title::{
        infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
        lookup_savedata_sync_item_for_title,
    },
    db::StateDb,
};

#[derive(Serialize, Deserialize)]
pub struct TitleDb(PathBuf, HashMap<u64, TitleDetails>);

impl TitleDb {
    pub fn open(root_path: impl AsRef<Path>) -> Result<Self> {
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
        log::info!("building runtime title db");

        let mut title_db = Self(root_path.as_ref().join("title.db"), HashMap::new());
        title_db.refresh(state_db, ui_tx)?;

        Ok(title_db)
    }

    pub fn refresh(&mut self, state_db: &StateDb, ui_tx: &Sender<UiMsg>) -> Result<()> {
        log::info!("refreshing all titles");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());

        let current_title_ids = SD_APP_TITLES.keys().copied().collect::<HashSet<_>>();
        self.1.retain(|k, _| current_title_ids.contains(k));

        let total = SD_APP_TITLES.len();
        for (i, title_id) in SD_APP_TITLES.keys().enumerate() {
            self.add_title(*title_id, state_db)?;
            self.refresh_links(*title_id, state_db)?;
            refresh_progress
                .message("Refreshing titles")
                .progress((i + 1) * 100 / total)
                .send();
        }

        Ok(())
    }

    fn add_title(&mut self, title_id: u64, state_db: &StateDb) -> Result<()> {
        let Some(title) = SD_APP_TITLES.get(&title_id) else {
            bail!("cannot find title {title_id:016X}");
        };

        let title_id = title.title_id;
        let product_code = &title.product_code;
        let smdh = ctr_title::smdh(title_id)?;

        log::info!("processing {title_id:016X}");

        let title = TitleDetails::new(title_id, &product_code, &smdh, &state_db);

        if title.savedata_sync_status != TitleSyncStatus::Unavailable
            || title.extdata_sync_status != TitleSyncStatus::Unavailable
        {
            log::debug!("added {title_id:016X}");
            self.1.insert(title_id, title);
        } else {
            log::debug!("ignored {title_id:016X}, has no save or extdata");
        }

        Ok(())
    }

    pub fn refresh_links(&mut self, title_id: u64, state_db: &StateDb) -> Result<()> {
        let extdata_sync_item = self.1.get(&title_id).and_then(|t| t.extdata_sync_item);

        for title in self
            .1
            .values_mut()
            .filter(|t| t.extdata_sync_item == extdata_sync_item)
        {
            title.refresh(state_db);
        }

        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        fs::write(&self.0, postcard::to_allocvec(&self)?)?;

        Ok(())
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
    pub savedata_sync_status: TitleSyncStatus,
    pub extdata_sync_status: TitleSyncStatus,
}

#[derive(Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub enum TitleSyncStatus {
    Unavailable,
    Available,
    Enabled,
    Disabled,
}

impl Display for TitleSyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TitleSyncStatus::Available => unreachable!(),
            TitleSyncStatus::Unavailable => write!(f, "Not available"),
            TitleSyncStatus::Enabled => write!(f, "Yes"),
            TitleSyncStatus::Disabled => write!(f, "No"),
        }
    }
}

impl TitleDetails {
    pub fn new(title_id: u64, product_code: &str, smdh: &CtrSmdh, state_db: &StateDb) -> Self {
        let savedata_sync_item = lookup_savedata_sync_item_for_title(title_id);
        let extdata_sync_item = lookup_extdata_sync_item_for_title(title_id)
            .or_else(|| infer_extdata_sync_item_for_title(title_id));

        let (sss, ess) = Self::sync_items_status(&savedata_sync_item, &extdata_sync_item, state_db);

        Self {
            title_id,
            product_code: product_code.to_string(),
            title_short: smdh.title_short(SmdhLanguage::English).to_string(),
            title_publisher: smdh.title_publisher(SmdhLanguage::English).to_string(),
            savedata_sync_item,
            extdata_sync_item,
            savedata_sync_status: sss,
            extdata_sync_status: ess,
        }
    }

    pub fn smdh(&self) -> Result<CtrSmdh> {
        Ok(ctr_title::smdh(self.title_id)?)
    }

    pub fn refresh(&mut self, state_db: &StateDb) {
        let (sss, ess) =
            Self::sync_items_status(&self.savedata_sync_item, &self.extdata_sync_item, state_db);

        self.savedata_sync_status = sss;
        self.extdata_sync_status = ess;
    }

    fn sync_items_status(
        savedata_sync_item: &Option<SyncItem>,
        extdata_sync_item: &Option<SyncItem>,
        state_db: &StateDb,
    ) -> (TitleSyncStatus, TitleSyncStatus) {
        let lookup = |si: &Option<SyncItem>| match si {
            Some(si) => match state_db.state(si) {
                Some(s) => match s.auto_enabled {
                    true => TitleSyncStatus::Enabled,
                    false => TitleSyncStatus::Disabled,
                },
                None => TitleSyncStatus::Available,
            },
            None => TitleSyncStatus::Unavailable,
        };

        let save = lookup(savedata_sync_item);
        let extdata = lookup(extdata_sync_item);

        (save, extdata)
    }
}
