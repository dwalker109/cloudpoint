use anyhow::Result;
use ctru::services::{am::Am, apt::Apt, gfx::Gfx, hid::Hid, romfs::RomFS, soc::Soc};

pub struct CtrServices {
    pub apt: Apt,
    pub hid: Hid,
    pub _am: Am,
    pub _rom: RomFS,
    pub _soc: Soc,
    pub _gfx: Gfx,
}

impl CtrServices {
    pub fn init() -> Result<Self> {
        log::debug!("initialising all ctr services");

        let apt = Apt::new()?;
        let hid = Hid::new()?;
        let _am = Am::new()?;
        let _rom = RomFS::new()?;
        let _soc = Soc::new()?;
        let _gfx = Gfx::new()?;

        Ok(Self {
            apt,
            hid,
            _am,
            _rom,
            _soc,
            _gfx,
        })
    }
}
