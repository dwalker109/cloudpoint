use crate::config::AppPath;
use crate::ctr_title::{
    SD_APP_TITLES_HASH, infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
    lookup_savedata_sync_item_for_title,
};
use crate::{
    app::{RefreshProgress, UiMsg},
    ctr_title::{SD_APP_TITLES, lookup_gba_sync_item_for_title, title_smdh},
};
use anyhow::{Context, Result, bail};
use cloudpoint_lib::utils::ellipsis;
use cloudpoint_lib::{
    ctr::{CtrSmdh, SmdhLanguage},
    sync::{SyncItem, SyncItemStatus, SyncState},
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::LazyLock;
use std::{collections::HashMap, fs, path::PathBuf, sync::mpsc::Sender};

mod legacy;

const CURRENT_VERSION: u16 = 1;
static PATH: LazyLock<PathBuf> = LazyLock::new(|| AppPath::Db.join("title.db"));

#[derive(Serialize, Deserialize)]
pub struct TitleDb(HashMap<u64, TitleDetails>, BTreeSet<u64>);

impl TitleDb {
    pub fn open() -> Result<Self> {
        log::debug!("loading title db from disk");

        Ok(Self::decode(
            &fs::read(&*PATH).context("title db not found")?,
        )?)
    }

    fn decode(buf: &[u8]) -> Result<Self> {
        let (ver, data) = super::decode_parts(buf)?;

        Ok(match ver {
            CURRENT_VERSION => postcard::from_bytes(data)?,
            0 => postcard::from_bytes::<legacy::TitleDbV0>(data)?.into(),
            v => bail!("cannot decode title db version {v}"),
        })
    }

    pub fn commit(&mut self) -> Result<()> {
        log::debug!("saving title db to disk");

        super::write_to_disk(&*PATH, CURRENT_VERSION, &self)
    }

    pub fn new(ui_tx: &Sender<UiMsg>) -> Result<Self> {
        log::debug!("building new title db");

        let mut title_db = Self(HashMap::new(), BTreeSet::new());
        title_db.refresh(ui_tx);

        Ok(title_db)
    }

    pub fn refresh(&mut self, ui_tx: &Sender<UiMsg>) {
        log::debug!("refreshing title db records");

        let mut refresh_progress = RefreshProgress::new(ui_tx.clone());
        let total = SD_APP_TITLES.len();

        for (i, title_id) in SD_APP_TITLES.keys().enumerate() {
            match self.handle_title(*title_id) {
                Ok(_) => log::info!("processed title {title_id:016X}"),
                Err(e) => match self.0.remove(&title_id) {
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

        self.0.retain(|k, _| SD_APP_TITLES_HASH.contains(k));
        self.1 = SD_APP_TITLES_HASH.clone();
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

        match self.0.get_mut(&title_id) {
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

                self.0.insert(title_id, title);
            }
        }

        Ok(())
    }

    pub fn is_stale(&self) -> bool {
        self.1 != *SD_APP_TITLES_HASH
    }

    pub fn title_mut(&mut self, title_id: u64) -> Option<&mut TitleDetails> {
        self.0.get_mut(&title_id)
    }

    pub fn titles(&self) -> impl Iterator<Item = &TitleDetails> {
        self.0.values()
    }

    pub fn titles_qty(&self) -> usize {
        self.0.len()
    }

    pub fn titles_sorted_vec(&self) -> Vec<TitleDetails> {
        self.0
            .values()
            .sorted_by_key(|t| t.title_short.to_lowercase())
            .cloned()
            .collect()
    }

    pub fn sync_state_label(&self, sync_state: &SyncState) -> String {
        let (mut t, mut p) = (Vec::new(), Vec::new());

        for title in self
            .0
            .values()
            .filter(|t| sync_state.via_title_ids.contains(&t.title_id))
        {
            t.push(title.title_short.clone());
            p.push(title.title_publisher.clone());
        }

        ellipsis(&format!("{} ({})", t.join("/"), p.join("/")), 35)
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
