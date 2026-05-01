use crate::config::AppPath;
use anyhow::Result;
use ctru::{console::Console, services::hid::KeyPad, set_panic_hook};
use ctru_sys::{InfoLedPattern, MCUHWC_SetInfoLedPattern, mcuHwcExit, mcuHwcInit};

pub mod app_logger;
pub mod config;
mod ctr_fs;
pub mod ctr_ndmu;
pub mod db;
mod services;
mod setup;
mod sync;
mod tree;

fn main() -> Result<()> {
    let _logger = app_logger::AppLogger::new()?;
    setup::sdmc()?;

    set_panic_hook(false);
    let mut sys_services = services::CtrSysServices::init()?;
    let gfx_services = services::CtrGfxServices::init()?;
    let _console = Console::new(gfx_services.gfx.top_screen.borrow_mut());

    let mut state_db = db::StateDb::open(AppPath::Db, &sys_services)?;

    println!("\x1b[20CCloudpoint\n");
    println!(
        "Ready to sync {} states across {} titles",
        state_db.total_states(),
        state_db.total_titles(),
    );
    println!("Press (A) to sync");
    println!("Press (X) to autodiscover");
    println!("Press Start to exit");

    while sys_services.apt.main_loop() {
        gfx_services.gfx.wait_for_vblank();

        sys_services.hid.scan_input();

        if sys_services.hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if sys_services.hid.keys_down().contains(KeyPad::A) {
            if let Err(e) = sys_services.ac.wait_internet_connection() {
                log::error!("unable to sync, no internet connection: {e}");
                println!("Not connected to the internet - connect and retry");

                continue;
            };

            let res = sync::run(&mut sys_services, &gfx_services, &mut state_db);

            if res.is_err() {
                log::error!("sync error: {res:?}");
            }

            println!("Results: {:?}", res);

            unsafe {
                mcuHwcInit();

                let steps = [
                    0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ];

                let pattern = InfoLedPattern {
                    delay: 0x02,
                    smoothing: 0x20,
                    loopDelay: 0x60,
                    blinkSpeed: 0x00,
                    redPattern: steps,
                    greenPattern: steps,
                    bluePattern: steps,
                };
                MCUHWC_SetInfoLedPattern(&pattern);
                mcuHwcExit();
            }
        }

        if sys_services.hid.keys_down().contains(KeyPad::X) {
            let res = state_db
                .append_discovered(&mut sys_services)
                .and_then(|_| state_db.save_all());

            if res.is_err() {
                log::error!("autodiscover error: {res:?}");
            }

            println!("Results: {:?}", res);
        }
    }

    unsafe {
        mcuHwcInit();

        let steps = [0x00; 32];

        let pattern = InfoLedPattern {
            delay: 0x00,
            smoothing: 0x00,
            loopDelay: 0x00,
            blinkSpeed: 0x00,
            redPattern: steps,
            greenPattern: steps,
            bluePattern: steps,
        };
        MCUHWC_SetInfoLedPattern(&pattern);
        mcuHwcExit();
    }

    Ok(())
}
