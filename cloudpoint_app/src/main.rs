use anyhow::Result;
use ctru::{console::Console, services::hid::KeyPad, set_panic_hook};
use std::collections::HashSet;

mod ctr_fs;
pub mod db;
mod services;
pub mod settings;
mod setup;
mod sync;
mod tree;

fn main() -> Result<()> {
    set_panic_hook(false);

    setup::logging()?;
    setup::sdmc()?;

    let mut sys_services = services::CtrSysServices::init()?;
    let gfx_services = services::CtrGfxServices::init()?;
    let _console = Console::new(gfx_services.gfx.top_screen.borrow_mut());

    let mut state_db = db::StateDb::open("sdmc:/3ds/Cloudpoint/db", &sys_services)?;

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
            let res = sync::run(&mut sys_services, &gfx_services, &mut state_db);

            if res.is_err() {
                log::error!("Error occurred during sync: {res:?}");
            }

            println!("Results: {:?}", res);
        }

        if sys_services.hid.keys_down().contains(KeyPad::X) {
            let res = setup::append_discovered(&mut sys_services, &mut state_db)
                .and_then(|_| state_db.save_all());

            if res.is_err() {
                log::error!("Error occurred during autodiscover: {res:?}");
            }

            println!("Results: {:?}", res);
        }
    }

    Ok(())
}
