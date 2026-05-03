use anyhow::Result;
use cloudpoint_lib::ctr::{CtrArchiveId, CtrMeta};

use ffi::{ctr_get_title_version, ctr_getr_ext_data_id_for_title};

pub fn meta(archive_id: CtrArchiveId) -> Result<CtrMeta> {
    match archive_id {
        CtrArchiveId::Savedata(title_id) => Ok(CtrMeta::new(ctr_get_title_version(title_id)?)),
        CtrArchiveId::Extdata(_) => Ok(CtrMeta::new(0)),
    }
}

pub fn extdata_archive_id_for_title(title_id: u64) -> Option<CtrArchiveId> {
    ctr_getr_ext_data_id_for_title(title_id)
        .ok()
        .and_then(|extdata_id| Some(CtrArchiveId::Extdata(extdata_id)))
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

        if R_FAILED(res) {
            bail!(
                "could not retrieve extdata_id for title {:016X} [{:#010X}]",
                title_id,
                res
            );
        }

        Ok(extdata_id)
    }
}
