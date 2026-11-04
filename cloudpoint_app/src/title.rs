use crate::ctr_fs::{CtrArchive, smdh};
use anyhow::Result;
use cloudpoint_lib::{
    ctr::{CtrMeta, CtrSmdh},
    sync::SyncItem,
};
use ctru::services::{
    am::{Am, Title},
    fs::MediaType,
};
use ffi::{ctr_get_ext_data_id_for_title, ctr_get_title_version};
use serde::Deserialize;
use std::{
    collections::{BTreeSet, HashMap},
    fs::read_to_string,
    path::PathBuf,
    sync::LazyLock,
};

pub fn program_id() -> u64 {
    ffi::apt_get_program_id().expect("should retrieve title id of self")
}

pub struct CtrAmTitle {
    pub title_id: u64,
    pub product_code: String,
}

impl<'a> From<&ctru::services::am::Title<'a>> for CtrAmTitle {
    fn from(value: &Title<'a>) -> Self {
        Self {
            title_id: value.id(),
            product_code: value.product_code().trim_end_matches('\0').to_string(),
        }
    }
}

#[derive(Default, Deserialize)]
struct TitleConfig {
    skip: Vec<u64>,
}

pub static SD_APP_TITLES: LazyLock<HashMap<u64, CtrAmTitle>> = LazyLock::new(|| {
    log::info!("building cached list of titles on SD");

    let am = Am::new().expect("am service should be available");

    let title_list = am
        .title_list(MediaType::Sd)
        .expect("am title list should be available");

    let title_config: TitleConfig = toml::from_str(
        &read_to_string("romfs:/title_config.toml")
            .expect("romfs:/title_config.toml should be present"),
    )
    .unwrap_or_default();

    let applications = title_list
        .iter()
        .filter(|t| (t.id() >> 32) as u32 == 0x00040000)
        .filter(|t| !title_config.skip.contains(&t.id()))
        .map(|t| (t.id(), CtrAmTitle::from(t)))
        .collect();

    applications
});

pub static SD_APP_TITLES_HASH: LazyLock<BTreeSet<u64>> =
    LazyLock::new(|| SD_APP_TITLES.keys().copied().collect());

static SD_TMD_ROOT: LazyLock<Option<PathBuf>> =
    LazyLock::new(|| ffi::fsuser_get_sdmc_ctr_root_path().ok());

pub fn title_smdh(title_id: u64) -> Result<CtrSmdh> {
    log::debug!("looking up smdh for {title_id} via faked SyncItem");

    let fake_sync_item = SyncItem::Savedata(title_id);
    Ok(smdh(fake_sync_item)?.into())
}

pub fn meta(sync_item: SyncItem) -> Result<CtrMeta> {
    log::debug!("looking up ctr meta for {sync_item}");

    match sync_item {
        SyncItem::Savedata(title_id) => Ok(CtrMeta::new(ctr_get_title_version(title_id)?)),
        SyncItem::Extdata(_) | SyncItem::Gba(_) => Ok(CtrMeta::new(0)),
    }
}

pub fn lookup_savedata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    log::debug!("looking up ctr savedata for title {title_id:016X} by probing save archive");

    let maybe_archive_id = SyncItem::Savedata(title_id);

    CtrArchive::open(maybe_archive_id)
        .map(|_| maybe_archive_id)
        .ok()
}

pub fn lookup_gba_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    log::debug!("looking up gba savedata for title {title_id:016X} by probing save archive");

    let maybe_archive_id = SyncItem::Gba(title_id);

    CtrArchive::open(maybe_archive_id)
        .map(|_| maybe_archive_id)
        .ok()
}

pub fn lookup_extdata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    ctr_get_ext_data_id_for_title(title_id)
        .ok()
        .and_then(|extdata_id| {
            log::debug!(
                "looking up extdata for title {title_id:016X} by probing title reported extdata id"
            );

            let maybe_archive_id = SyncItem::Extdata(extdata_id);

            CtrArchive::open(maybe_archive_id)
                .map(|_| maybe_archive_id)
                .ok()
        })
}

pub fn infer_extdata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    log::debug!("looking up extdata for title {title_id:016X} by inference");

    let maybe_archive_id = SyncItem::Extdata((title_id >> 8) & 0x00000000FFFFFFFF);

    CtrArchive::open(maybe_archive_id)
        .map(|_| maybe_archive_id)
        .ok()
}

pub fn get_installed_at_for_title(title_id: u64) -> u64 {
    let Some(root) = &*SD_TMD_ROOT else {
        log::warn!("tmd inspection unsupported on this platform");
        return 0;
    };

    let tmd_path = format!(
        "{}/title/00040000/{:08x}/content/00000000.tmd",
        root.display(),
        title_id as u32
    );

    match ffi::ctr_archive_get_mtime(&tmd_path) {
        Ok(mtime) => mtime,
        Err(e) => {
            log::warn!("tmd inspection failed for {title_id:016X}: {e}");
            0
        }
    }
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru::services::fs::MediaType;
    use ctru_sys::{
        AM_GetTitleExtDataId, AM_GetTitleInfo, AM_TitleInfo, APT_GetProgramID,
        FSUSER_GetSdmcCtrRootPath, MEDIATYPE_SD, R_FAILED, archive_getmtime,
    };
    use std::{ffi::CString, path::PathBuf};

    pub(super) fn apt_get_program_id() -> Result<u64> {
        let mut id = 0;

        let res = unsafe { APT_GetProgramID(&mut id) };

        if R_FAILED(res) {
            bail!("could not get title id Cloudpoint is running under")
        }

        Ok(id)
    }

    pub(super) fn fsuser_get_sdmc_ctr_root_path() -> Result<PathBuf> {
        let mut buf = [0u8; 0xA0];
        let res = unsafe { FSUSER_GetSdmcCtrRootPath(buf.as_mut_ptr(), buf.len() as u32) };

        if R_FAILED(res) || buf[0] == 0x00 {
            bail!("could not get sdmc ctr root path");
        }

        Ok([
            "sdmc:/".into(),
            String::from_utf16le(&buf[2..buf.len() - 2])?,
        ]
        .into_iter()
        .collect())
    }

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

    pub(super) fn ctr_get_ext_data_id_for_title(title_id: u64) -> Result<u64> {
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

    pub(super) fn ctr_archive_get_mtime(path: &str) -> Result<u64> {
        let mut mtime: u64 = 0;

        let res = unsafe { archive_getmtime(CString::new(path)?.as_ptr(), &mut mtime) };

        if R_FAILED(res) {
            bail!("could not retreive mtime for {:?}", path);
        }

        Ok(mtime)
    }
}
