use crate::config::APP_VER;
use anyhow::{Result, bail};
use cloudpoint_lib::{http::CurlHttpClient, preflight::ClientUpdate};
use ctru_sys::Handle;
use sha2::Digest;

pub fn self_update(update: &ClientUpdate) -> Result<()> {
    let req = CurlHttpClient::new(&*APP_VER)?.get(&update.url, &[])?;

    if req.status != 200 {
        bail!("could not download CIA file");
    }

    if req.body.len() != update.size {
        bail!(
            "downloaded CIA is the wrong size ({} received, expected {})",
            req.body.len(),
            update.size
        );
    }

    let su_sha256 = hex::decode(update.digest.strip_prefix("sha256:").unwrap_or_default())?;

    let mut hasher = sha2::Sha256::new();
    hasher.update(&req.body);
    let dl_sha256 = hasher.finalize();

    if su_sha256.as_slice() != dl_sha256.as_slice() {
        bail!("downloaded CIA is invalid");
    }

    SelfUpdate::install(&req.body)
}

struct SelfUpdate<'a>(&'a [u8], Handle);

impl<'a> SelfUpdate<'a> {
    pub fn install(data: &[u8]) -> Result<()> {
        let mut su = SelfUpdate(data, ffi::start_cia_install()?);
        ffi::write_cia_install(su.0, su.1)?;
        ffi::finish_cia_install(su.1)?;
        su.1 = 0;

        Ok(())
    }
}

impl<'a> Drop for SelfUpdate<'a> {
    fn drop(&mut self) {
        if self.1 != 0 {
            log::debug!("CIA install dropped with a held file handle");
            ffi::cancel_cia_install(self.1);
        }
    }
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru_sys::{
        AM_CancelCIAInstall, AM_FinishCiaInstall, AM_StartCiaInstall, FS_WRITE_FLUSH, FSFILE_Write,
        Handle, MEDIATYPE_SD, R_FAILED,
    };
    use std::os::raw::c_void;

    pub(super) fn start_cia_install() -> Result<Handle> {
        log::debug!("starting CIA install for self update");

        let mut h: Handle = 0;
        let res = unsafe { AM_StartCiaInstall(MEDIATYPE_SD, &mut h) };

        if R_FAILED(res) {
            bail!("could not start CIA install: {res}");
        }

        Ok(h)
    }

    pub(super) fn write_cia_install(cia: &[u8], h: Handle) -> Result<()> {
        log::debug!(
            "writing {} bytes for CIA install via file handle {h}",
            cia.len()
        );

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
        log::debug!("finishing/committing CIA install via file handle {h}");

        let res = unsafe { AM_FinishCiaInstall(h) };

        if R_FAILED(res) {
            bail!("could not finish/commit CIA install: {res}");
        }

        Ok(())
    }

    pub(super) fn cancel_cia_install(h: Handle) {
        log::debug!("cancelling CIA install via file handle {h}");

        let res = unsafe { AM_CancelCIAInstall(h) };

        if R_FAILED(res) {
            log::warn!("could not cancel CIA install: {res}");
        }
    }
}
