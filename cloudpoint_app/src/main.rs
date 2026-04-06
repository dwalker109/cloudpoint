use anyhow::Result;
use ctru::{console::Console, services::hid::KeyPad, set_panic_hook};
use std::collections::HashSet;

mod ctr_fs;
mod services;
mod settings;
mod setup;
mod store;
mod sync;
mod tree;

fn main() -> Result<()> {
    set_panic_hook(false);

    setup::logging()?;
    setup::sdmc()?;

    let mut sys_services = services::CtrSysServices::init()?;
    let gfx_services = services::CtrGfxServices::init()?;
    let _console = Console::new(gfx_services.gfx.top_screen.borrow_mut());

    let mut sync_states = setup::sync_states(&sys_services)?;

    println!("\x1b[20CCloudpoint\n");
    println!(
        "Ready to sync {} states across {} titles",
        sync_states.len(),
        sync_states
            .values()
            .map(|s| s.title_id)
            .collect::<HashSet<_>>()
            .len(),
    );
    println!("Press (A) to sync");
    println!("Press Start to exit");

    while sys_services.apt.main_loop() {
        gfx_services.gfx.wait_for_vblank();

        sys_services.hid.scan_input();

        if sys_services.hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if sys_services.hid.keys_down().contains(KeyPad::A) {
            let res = sync::run(&mut sys_services, &gfx_services, &mut sync_states);

            if res.is_err() {
                log::error!("Error occurred during sync: {res:?}");
            }

            println!("Results: {:?}", res);
        }
    }

    Ok(())
}
