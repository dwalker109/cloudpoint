#![feature(oneshot_channel)]

mod app;
pub mod app_logger;
pub mod config;
mod ctr_fs;
pub mod ctr_gfx;
pub mod ctr_ndmu;
pub mod ctr_title;
pub mod db;
pub mod screens;
mod services;
mod setup;
mod sync;
mod tree;

use anyhow::Result;

fn main() -> Result<()> {
    ctru::set_panic_hook(false);
    setup::sdmc()?;

    let _logger = app_logger::AppLogger::new()?;
    let services = services::CtrServices::init()?;

    app::App::run(services)?;

    Ok(())
}
