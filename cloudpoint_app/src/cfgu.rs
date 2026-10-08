use anyhow::Result;
use ctru::services::cfgu::{Cfgu, SystemModel};

pub struct NewMode(());

impl NewMode {
    pub fn new() -> Result<Self> {
        match Cfgu::new()?.model()? {
            SystemModel::Old3DS | SystemModel::Old3DSXL | SystemModel::Old2DS => {
                log::debug!(r#"model is "Old" family, no perf increase available"#);
                ffi::ctr_os_set_speedup_enable(false);
            }
            SystemModel::New3DS | SystemModel::New3DSXL | SystemModel::New2DSXL => {
                log::debug!(r#"model is "New" family, enabling enhanced CPU clock and L2 cache"#);
                ffi::ctr_os_set_speedup_enable(true);
            }
        }

        Ok(Self(()))
    }
}

impl Drop for NewMode {
    fn drop(&mut self) {
        ffi::ctr_os_set_speedup_enable(false);
    }
}

pub(super) mod ffi {
    pub fn ctr_os_set_speedup_enable(enable: bool) {
        unsafe {
            ctru_sys::osSetSpeedupEnable(enable);
        }
    }
}
