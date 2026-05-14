use crate::ctr_title::{self, SD_APP_TITLES};
use anyhow::Result;
use itertools::Itertools;
use std::{collections::HashMap, fmt::Display, sync::Arc};

use cloudpoint_lib::{ctr::CtrSmdh, sync::SyncItem};

use crate::{
    ctr_title::{
        infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
        lookup_savedata_sync_item_for_title,
    },
    db::StateDb,
};

pub struct TitleDb(HashMap<u64, TitleDetails>);

impl TitleDb {
    pub fn build(state_db: &StateDb) -> Result<Self> {
        log::info!("building runtime title db");

        let mut titles = HashMap::new();

        for title in SD_APP_TITLES.iter() {
            let title_id = title.title_id;
            let product_code = &title.product_code;
            let smdh = ctr_title::smdh(title_id)?;

            log::info!("processing {title_id:016X}");

            let title = TitleDetails::new(title_id, &product_code, smdh, &state_db);

            if title.savedata_sync_status != TitleSyncStatus::Unavailable
                || title.extdata_sync_status != TitleSyncStatus::Unavailable
            {
                log::debug!("added {title_id:016X}");
                titles.insert(title_id, title);
            } else {
                log::debug!("ignored {title_id:016X}, has no save or extdata");
            }
        }

        Ok(Self(titles))
    }

    pub fn total_titles(&self) -> usize {
        self.0.len()
    }

    pub fn titles_sorted_vec(&self) -> Vec<TitleDetails> {
        self.0
            .values()
            .sorted_by_key(|t| {
                t.smdh
                    .title_short(cloudpoint_lib::ctr::SmdhLanguage::English)
            })
            .cloned()
            .collect()
    }

    pub fn refresh_cascade(&mut self, title_id: u64, state_db: &StateDb) -> Result<()> {
        let extdata_sync_item = self.0.get(&title_id).and_then(|t| t.extdata_sync_item);

        for title in self
            .0
            .values_mut()
            .filter(|t| t.extdata_sync_item == extdata_sync_item)
        {
            title.refresh(state_db);
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct TitleDetails {
    pub title_id: u64,
    pub product_code: String,
    pub smdh: Arc<CtrSmdh>,
    pub savedata_sync_item: Option<SyncItem>,
    pub extdata_sync_item: Option<SyncItem>,
    pub savedata_sync_status: TitleSyncStatus,
    pub extdata_sync_status: TitleSyncStatus,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TitleSyncStatus {
    Unavailable,
    Available,
    Enabled,
    Disabled,
}

impl Display for TitleSyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TitleSyncStatus::Unavailable => write!(f, "Unavailable"),
            TitleSyncStatus::Available => write!(f, "Available"),
            TitleSyncStatus::Enabled => write!(f, "Enabled"),
            TitleSyncStatus::Disabled => write!(f, "Disabled"),
        }
    }
}

impl TitleDetails {
    pub fn new(title_id: u64, product_code: &str, smdh: CtrSmdh, state_db: &StateDb) -> Self {
        let savedata_sync_item = lookup_savedata_sync_item_for_title(title_id);
        let extdata_sync_item = lookup_extdata_sync_item_for_title(title_id)
            .or_else(|| infer_extdata_sync_item_for_title(title_id));

        let (sss, ess) = Self::sync_items_status(&savedata_sync_item, &extdata_sync_item, state_db);

        Self {
            title_id,
            product_code: product_code.to_string(),
            smdh: Arc::new(smdh),
            savedata_sync_item,
            extdata_sync_item,
            savedata_sync_status: sss,
            extdata_sync_status: ess,
        }
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
