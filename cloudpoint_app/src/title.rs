use std::fmt::Display;

use cloudpoint_lib::{ctr::CtrSmdh, sync::SyncItem};

use crate::{
    ctr_title::{
        infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title,
        lookup_savedata_sync_item_for_title,
    },
    db::StateDb,
};

pub struct TitleDetails {
    pub title_id: u64,
    pub product_code: String,
    pub smdh: CtrSmdh,
    pub savedata_sync_item: Option<SyncItem>,
    pub savedata_sync_status: TitleSyncStatus,
    pub extdata_sync_item: Option<SyncItem>,
    pub extdata_sync_status: TitleSyncStatus,
}

#[derive(Eq, PartialEq)]
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
        let (savedata_sync_status, extdata_sync_status) =
            Self::sync_items_status(&savedata_sync_item, &extdata_sync_item, state_db);

        Self {
            title_id,
            product_code: product_code.trim_end_matches('\0').to_string(),
            smdh,
            savedata_sync_item,
            savedata_sync_status,
            extdata_sync_item,
            extdata_sync_status,
        }
    }

    fn sync_items_status(
        savedata_sync_item: &Option<SyncItem>,
        extdata_sync_item: &Option<SyncItem>,
        state_db: &StateDb,
    ) -> (TitleSyncStatus, TitleSyncStatus) {
        let lookup = |si: &Option<SyncItem>| match si {
            Some(si) => match state_db.state(si) {
                Some(s) => match s.enabled {
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
