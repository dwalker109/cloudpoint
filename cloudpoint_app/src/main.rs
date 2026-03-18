use anyhow::{Context, Result};
use cloudpoint_lib::title::SyncState;
use ctru::console::Console;
use ctru::services::am::Title;
use ctru::services::fs::MediaType;
use ctru::services::hid::KeyPad;
use ctru::services::{am::Am, apt::Apt, gfx::Gfx, hid::Hid, soc::Soc};
use std::fs;
use std::fs::{create_dir_all, File};

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let _net = Soc::new()?;

    setup_sdmc()?;
    let all_sync_states = get_sync_states()?;
    let installed_titles = get_installed_titles(&am)?;
    let active_sync_states = prune_sync_states(&all_sync_states, &installed_titles);

    println!("Active sync states: {:?}", active_sync_states);
    println!("Press (A) to sync");
    println!("Press Start to exit");

    while apt.main_loop() {
        gfx.wait_for_vblank();

        hid.scan_input();

        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if hid.keys_down().contains(KeyPad::A) {
            let results = do_sync(&active_sync_states);
            println!("Results: {:?}", results);
        }
    }

    Ok(())
}

fn setup_sdmc() -> Result<()> {
    let path = "sdmc:/3ds/Cloudpoint";
    create_dir_all(path).with_context(|| format!("failed to create working directory {path}"))?;

    Ok(())
}

fn get_sync_states() -> Result<Vec<SyncState>> {
    let mut configs: Vec<SyncState> = Vec::new();
    for f in fs::read_dir("sdmc:/3ds/Cloudpoint")? {
        let f = f?;
        let s: SyncState = serde_json::from_reader(File::open(f.path())?)?;
    }

    Ok(vec![])
}

fn get_installed_titles(am: &Am) -> Result<Vec<Title>> {
    let titles = am.title_list(MediaType::Sd)?;

    Ok(titles)
}

fn prune_sync_states(sync_states: &[SyncState], installed_titles: &[Title]) -> Vec<SyncState> {
    let ids = installed_titles.iter().map(|t| t.id()).collect::<Vec<_>>();
    sync_states
        .iter()
        .filter(|&s| ids.contains(&s.title_id))
        .collect()
}

fn do_sync(active_sync_states: &[SyncState]) -> Result<Vec<(u64, String, String)>> {
    let mut results = Vec::new();

    for s in active_sync_states {
        let v = cloudpoint_lib::version::VersionDirList::try_get(
            "http://192.168.1.163:8080",
            "dw",
            s.title_id,
        )?
        .latest();



        match s.get_action() {
            SyncAction::Nothing => {}
            SyncAction::Conflict => {}
            SyncAction::Upload => {}
            SyncAction::Download => {}
        }
    }

    Ok(results)
}
