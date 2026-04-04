use anyhow::Result;
use ctru::services::{am::Am, apt::Apt, gfx::Gfx, hid::Hid, soc::Soc};

pub struct CtrSysServices {
    pub am: Am,
    pub apt: Apt,
    pub hid: Hid,
    pub soc: Soc,
}

impl CtrSysServices {
    pub fn init() -> Result<Self> {
        let am = Am::new()?;
        let apt = Apt::new()?;
        let hid = Hid::new()?;
        let soc = Soc::new()?;

        Ok(Self { am, apt, hid, soc })
    }
}

pub struct CtrGfxServices {
    pub gfx: Gfx,
}

impl CtrGfxServices {
    pub fn init() -> Result<Self> {
        let gfx = Gfx::new()?;

        Ok(Self { gfx })
    }
}
