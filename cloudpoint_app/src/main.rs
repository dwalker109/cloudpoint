#![feature(oneshot_channel)]
#![feature(try_blocks)]
#![feature(string_from_utf8_lossy_owned)]

use crate::ctr_nwm::ForceWlan;
use anyhow::Result;

mod app;
mod app_logger;
mod config;
mod ctr_cfgi;
mod ctr_fs;

mod ctr_ndmu;
mod ctr_nwm;
mod ctr_os;
mod ctr_title;
mod db;
mod gfx;
mod link;
mod screens;
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
