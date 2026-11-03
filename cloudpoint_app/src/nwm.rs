use anyhow::Result;
use ctru::services::ac::{Ac, NetworkStatus};

use crate::config::USER_SETTINGS;

pub struct ForceWlan {
    launch_state: Option<NetworkStatus>,
}

impl ForceWlan {
    pub fn new() -> Result<Self> {
        let launch_state = match USER_SETTINGS.auto_wifi {
            true => {
                log::debug!("auto_wifi is enabled");
                Ac::new()?.wifi_status().ok()
            }
            false => {
                log::debug!("auto_wifi is disabled");
                None
            }
        };

        if matches!(
            launch_state,
            Some(NetworkStatus::None | NetworkStatus::Idle)
        ) {
            log::debug!("trying to enable wireless radio");
            ffi::ctr_control_wireless_enabled(true).ok();
        }

        Ok(Self { launch_state })
    }
}

impl Drop for ForceWlan {
    fn drop(&mut self) {
        if matches!(
            self.launch_state,
            Some(NetworkStatus::None | NetworkStatus::Idle)
        ) {
            log::debug!("trying to disable wireless radio to restore launch state");
            ffi::ctr_control_wireless_enabled(false).ok();
        }
    }
}

mod ffi {
    use anyhow::{Result, bail};
    use ctru_sys::{NWMEXT_ControlWirelessEnabled, R_FAILED, nwmExtExit, nwmExtInit};

    pub(super) fn ctr_control_wireless_enabled(enable: bool) -> Result<()> {
        let res = unsafe { nwmExtInit() };

        if R_FAILED(res) {
            bail!("could not initialise new_ext");
        }

        let res = unsafe { NWMEXT_ControlWirelessEnabled(enable) };

        if R_FAILED(res) {
            bail!("could not control wireless");
        }

        unsafe { nwmExtExit() };

        Ok(())
    }
}
