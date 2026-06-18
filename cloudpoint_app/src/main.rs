#![feature(oneshot_channel)]
#![feature(try_blocks)]

mod app;
pub mod app_logger;
pub mod config;
pub mod ctr_cfgi;
mod ctr_fs;
pub mod ctr_gfx;
pub mod ctr_ndmu;
pub mod ctr_title;
pub mod db;
mod link;
pub mod screens;
mod setup;
mod sync;
mod tree;

use anyhow::Result;

fn main() -> Result<()> {
    ctru::set_panic_hook(false);

    let _logger = app_logger::AppLogger::new()?;
    let _sdmc = setup::sdmc()?;
    let _ctr_svc = setup::ambient_ctr_services()?;

    app::App::run()?;

    Ok(())
}
