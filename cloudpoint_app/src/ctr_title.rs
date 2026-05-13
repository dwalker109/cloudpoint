use std::sync::LazyLock;

use crate::ctr_fs::CtrArchive;
use anyhow::Result;
use cloudpoint_lib::{
    ctr::{CtrMeta, CtrSmdh},
    sync::SyncItem,
};
use ctru::services::{
    am::{Am, Title},
    fs::MediaType,
};
use ffi::{ctr_get_title_version, ctr_getr_ext_data_id_for_title};

pub struct CtrAmTitle {
    pub title_id: u64,
    pub product_code: String,
    pub version: u16,
}

impl<'a> From<&ctru::services::am::Title<'a>> for CtrAmTitle {
    fn from(value: &Title<'a>) -> Self {
        Self {
            title_id: value.id(),
            product_code: value.product_code().trim_end_matches('\0').to_string(),
            version: value.version(),
        }
    }
}

pub static SD_APP_TITLES: LazyLock<Vec<CtrAmTitle>> = LazyLock::new(|| {
    let am = Am::new().expect("am service should be available");
    let title_list = am
        .title_list(MediaType::Sd)
        .expect("am title list should be available");
    let applications = title_list
        .iter()
        .filter(|t| (t.id() >> 32) as u32 == 0x00040000)
        .map(CtrAmTitle::from)
        .collect();

    applications
});

pub fn smdh(title_id: u64) -> Result<CtrSmdh> {
    // Fetching CtrSmdh for a Savedata sync item really fetches it from the title - no archive needs to exist
    Ok(CtrArchive::smdh(SyncItem::Savedata(title_id))?.into())
}

pub fn meta(sync_item: SyncItem) -> Result<CtrMeta> {
    match sync_item {
        SyncItem::Savedata(title_id) => Ok(CtrMeta::new(ctr_get_title_version(title_id)?)),
        SyncItem::Extdata(_) => Ok(CtrMeta::new(0)),
    }
}

pub fn lookup_savedata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    let maybe_archive_id = SyncItem::Savedata(title_id);

    CtrArchive::open(maybe_archive_id)
        .map(|_| maybe_archive_id)
        .ok()
}

pub fn lookup_extdata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    ctr_getr_ext_data_id_for_title(title_id)
        .ok()
        .and_then(|extdata_id| Some(SyncItem::Extdata(extdata_id)))
}

pub fn infer_extdata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    let maybe_archive_id = SyncItem::Extdata((title_id >> 8) & 0x00000000FFFFFFFF);

    CtrArchive::open(maybe_archive_id)
        .map(|_| maybe_archive_id)
        .ok()
}

mod ffi {
    use anyhow::Result;
    use anyhow::bail;
    use ctru::services::fs::MediaType;
    use ctru_sys::AM_GetTitleExtDataId;
    use ctru_sys::{AM_GetTitleInfo, AM_TitleInfo, MEDIATYPE_SD, R_FAILED};

    pub(super) fn ctr_get_title_version(title_id: u64) -> Result<u16> {
        let mut title_info: AM_TitleInfo = unsafe { std::mem::zeroed() };

        let res = unsafe {
            AM_GetTitleInfo(
                MEDIATYPE_SD,
                1,
                &title_id as *const u64 as _,
                &mut title_info,
            )
        };

        if R_FAILED(res) {
            bail!(
                "could not get title info for title {} [{:#010X}]",
                title_id,
                res
            );
        }

        Ok(title_info.version)
    }

    pub(super) fn ctr_getr_ext_data_id_for_title(title_id: u64) -> Result<u64> {
        let mut extdata_id: u64 = 0;

        let res = unsafe { AM_GetTitleExtDataId(&mut extdata_id, MediaType::Sd as u8, title_id) };

        if R_FAILED(res) || extdata_id == 0 {
            bail!(
                "could not retrieve extdata_id for title {:016X} (or it may not have one) [{:#010X}]",
                title_id,
                res
            );
        }

        Ok(extdata_id)
    }
}
