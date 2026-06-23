#![feature(oneshot_channel)]
#![feature(try_blocks)]
#![feature(string_from_utf8_lossy_owned)]

use crate::ctr_nwm::ForceWlan;
use anyhow::Result;

mod app;
pub mod app_logger;
pub mod config;
pub mod ctr_cfgi;
mod ctr_fs;
pub mod ctr_ndmu;
pub mod ctr_nwm;
mod ctr_os;
pub mod ctr_title;
pub mod db;
pub mod gfx;
mod link;
pub mod screens;
mod setup;
mod sync;
mod tree;

fn main() -> Result<()> {
    ctru::set_panic_hook(false);

    let _logger = app_logger::AppLogger::new()?;
    let _new_mode = ctr_os::NewMode::new()?;
    let _wlan = ForceWlan::new()?;
    let _sdmc = setup::sdmc()?;
    let _ctr_svc = setup::ambient_ctr_services()?;

    app::App::run()?;

    Ok(())
}
