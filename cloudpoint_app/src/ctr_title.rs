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
use ffi::{ctr_get_ext_data_id_for_title, ctr_get_title_version};
use std::{collections::HashMap, ffi::CString, fs::read_dir, path::PathBuf, sync::LazyLock};

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

pub static SD_APP_TITLES: LazyLock<HashMap<u64, CtrAmTitle>> = LazyLock::new(|| {
    log::info!("building cached list of titles on SD");

    let am = Am::new().expect("am service should be available");
    let title_list = am
        .title_list(MediaType::Sd)
        .expect("am title list should be available");
    let applications = title_list
        .iter()
        .filter(|t| (t.id() >> 32) as u32 == 0x00040000)
        .map(|t| (t.id(), CtrAmTitle::from(t)))
        .collect();

    applications
});

static SD_TMD_ROOTS: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    let mut roots = Vec::new();

    static EXPECT_MSG: &str =
        "sdmc:/Nintendo 3DS/<id0>/<id1>/ dirs should always exist and be readble";

    for entry in read_dir("sdmc:/Nintendo 3DS").expect(EXPECT_MSG) {
        let id0 = entry.expect(EXPECT_MSG);
        if id0.file_type().expect(EXPECT_MSG).is_dir() && id0.file_name().len() == 32 {
            for entry in read_dir(id0.path()).expect(EXPECT_MSG) {
                let id1 = entry.expect(EXPECT_MSG);
                if id1.file_type().expect(EXPECT_MSG).is_dir() && id1.file_name().len() == 32 {
                    roots.push(id1.path());
                }
            }
        }
    }

    roots
});

pub fn smdh(title_id: u64) -> Result<CtrSmdh> {
    log::debug!("looking up smdh for {title_id} via faked SyncItem");

    let fake_sync_item = SyncItem::Savedata(title_id);
    Ok(CtrArchive::smdh(fake_sync_item)?.into())
}

pub fn meta(sync_item: SyncItem) -> Result<CtrMeta> {
    log::debug!("looking up ctr meta for {sync_item}");

    match sync_item {
        SyncItem::Savedata(title_id) => Ok(CtrMeta::new(ctr_get_title_version(title_id)?)),
        SyncItem::Extdata(_) => Ok(CtrMeta::new(0)),
    }
}

pub fn lookup_savedata_sync_item_for_title(title_id: u64) -> Option<SyncItem> {
    log::debug!("looking up savedata for title {title_id:016X} by probing save archive");

    let maybe_archive_id = SyncItem::Savedata(title_id);

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

pub fn get_installed_at_for_title(title_id: u64) -> Result<u64> {
    let mut latest = 0;

    for root in &*SD_TMD_ROOTS {
        let tmd_path = CString::new(format!(
            "{}/title/00040000/{:08x}/content/00000000.tmd",
            root.display(),
            title_id as u32
        ))?;

        let mtime = ffi::ctr_archive_get_mtime(tmd_path)?;
        latest = latest.max(mtime);
    }

    Ok(latest)
}

mod ffi {
    use anyhow::Result;
    use anyhow::bail;
    use ctru::services::fs::MediaType;
    use ctru_sys::AM_GetTitleExtDataId;
    use ctru_sys::archive_getmtime;
    use ctru_sys::{AM_GetTitleInfo, AM_TitleInfo, MEDIATYPE_SD, R_FAILED};
    use std::ffi::CString;

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

    pub(super) fn ctr_archive_get_mtime(path: CString) -> Result<u64> {
        let mut mtime: u64 = 0;

        let res = unsafe { archive_getmtime(path.as_ptr(), &mut mtime) };

        if R_FAILED(res) {
            bail!("could not retreive mtime for {:?}", path);
        }

        Ok(mtime)
    }
}
