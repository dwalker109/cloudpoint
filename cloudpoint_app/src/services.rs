use anyhow::Result;
use ctru::services::{ac::Ac, am::Am, apt::Apt, gfx::Gfx, hid::Hid, romfs::RomFS, soc::Soc};

pub struct CtrSysServices {
    pub am: Am,
    pub apt: Apt,
    pub hid: Hid,
    pub ac: Ac,
    pub _rom: RomFS,
    pub _soc: Soc,
}

impl CtrSysServices {
    pub fn init() -> Result<Self> {
        let am = Am::new()?;
        let apt = Apt::new()?;
        let hid = Hid::new()?;
        let ac = Ac::new()?;
        let _rom = RomFS::new()?;
        let _soc = Soc::new()?;

        Ok(Self {
            am,
            apt,
            hid,
            ac,
            _rom,
            _soc,
        })
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
