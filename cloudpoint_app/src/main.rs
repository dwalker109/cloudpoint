#![feature(oneshot_channel)]
#![feature(string_from_utf8_lossy_owned)]
#![feature(io_const_error)]

mod app;
mod app_logger;
mod cfgi;
mod cfgu;
mod config;
mod ctr_fs;
mod db;
mod gfx;
mod link;
mod ndmu;
mod nwm;
mod pxi;
mod screens;
mod setup;
mod sync;
mod title;

fn main() -> anyhow::Result<()> {
    ctru::set_panic_hook(false);

    let _logger = app_logger::AppLogger::new()?;
    let _new_mode = cfgu::NewMode::new()?;
    let _wlan = nwm::ForceWlan::new()?;
    let _sdmc = setup::sdmc()?;
    let _ctr_svc = setup::ambient_ctr_services()?;

    app::App::run()?;

    Ok(())
}
