pub struct KeepAwake(());

impl KeepAwake {
    pub fn new() -> Self {
        log::debug!("entering sleep prevent context...");

        if let Err(e) = ffi::ctr_prevent_sleep() {
            log::warn!("could not prevent sleep: {e}");
        }

        Self(())
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        log::debug!("...existing sleep prevent context");

        if let Err(e) = ffi::ctr_allow_sleep() {
            log::warn!("could not allow sleep: {e}");
        }
    }
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru_sys::{
        NDM_EXCLUSIVE_STATE_INFRASTRUCTURE, NDMU_EnterExclusiveState, NDMU_LeaveExclusiveState,
        NDMU_LockState, NDMU_UnlockState, R_FAILED, aptSetSleepAllowed, ndmuExit, ndmuInit,
    };

    pub(super) fn ctr_prevent_sleep() -> Result<()> {
        let res = unsafe { ndmuInit() };

        if R_FAILED(res) {
            bail!("could not initialise ndmu");
        }

        unsafe { aptSetSleepAllowed(false) };

        let res1 = unsafe { NDMU_EnterExclusiveState(NDM_EXCLUSIVE_STATE_INFRASTRUCTURE) };
        let res2 = unsafe { NDMU_LockState() };

        if R_FAILED(res1) || R_FAILED(res2) {
            bail!("could not enter or lock ndmu exclusive state");
        }

        Ok(())
    }
    pub(super) fn ctr_allow_sleep() -> Result<()> {
        let res1 = unsafe { NDMU_UnlockState() };
        let res2 = unsafe { NDMU_LeaveExclusiveState() };

        if R_FAILED(res1) || R_FAILED(res2) {
            bail!("could not unlock or leave ndmu exclusive state");
        }

        unsafe { aptSetSleepAllowed(true) };

        unsafe { ndmuExit() };

        Ok(())
    }
}
