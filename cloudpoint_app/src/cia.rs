use crate::config::APP_VER;
use anyhow::{Result, bail};
use cloudpoint_lib::{http::CurlHttpClient, preflight::ClientUpdate};
use sha2::Digest;

pub fn self_update(update: &ClientUpdate) -> Result<()> {
    let cia = CurlHttpClient::new(&*APP_VER)?.get(&update.url, &[])?;

    if cia.status != 200 {
        bail!("could not download CIA file");
    }

    let su_sha256 = hex::decode(update.digest.strip_prefix("sha256:").unwrap_or_default())?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(&cia.body);
    let dl_sha256 = hasher.finalize();

    if su_sha256.as_slice() != dl_sha256.as_slice() {
        bail!("downloaded CIA is invalid");
    }

    let handle = ffi::start_cia_install()?;

    if let Err(e) = ffi::write_cia_install(&cia.body, handle) {
        ffi::cancel_cia_install(handle);

        return Err(e);
    }

    if let Err(e) = ffi::finish_cia_install(handle) {
        ffi::cancel_cia_install(handle);

        return Err(e);
    }

    Ok(())
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru_sys::{
        AM_CancelCIAInstall, AM_FinishCiaInstall, AM_StartCiaInstall, FS_WRITE_FLUSH, FSFILE_Write,
        Handle, MEDIATYPE_SD, R_FAILED,
    };
    use std::os::raw::c_void;

    pub(super) fn start_cia_install() -> Result<Handle> {
        let mut h: Handle = 0;
        let res = unsafe { AM_StartCiaInstall(MEDIATYPE_SD, &mut h) };

        if R_FAILED(res) {
            bail!("could not start CIA install: {res}");
        }

        Ok(h)
    }

    pub(super) fn write_cia_install(cia: &[u8], h: Handle) -> Result<()> {
        let mut bytes_written = 0;
        let res = unsafe {
            FSFILE_Write(
                h,
                &mut bytes_written,
                0,
                cia.as_ptr() as *const c_void,
                cia.len() as u32,
                FS_WRITE_FLUSH as u32,
            )
        };

        if R_FAILED(res) {
            bail!("could not write CIA data: {res}");
        }

        Ok(())
    }

    pub(super) fn finish_cia_install(h: Handle) -> Result<()> {
        let res = unsafe { AM_FinishCiaInstall(h) };

        if R_FAILED(res) {
            bail!("could not finish/commit CIA install: {res}");
        }

        Ok(())
    }

    pub(super) fn cancel_cia_install(h: Handle) {
        let res = unsafe { AM_CancelCIAInstall(h) };

        if R_FAILED(res) {
            log::warn!("could not cancel CIA install: {res}");
        }
    }
}
