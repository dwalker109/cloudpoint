mod archive;
mod store;

use anyhow::{Context, Result, anyhow};
use chunktree::tree::Tree;
use chunktree::version::Version;
use cloudpoint_lib::sync::{SyncAction, SyncState};
use ctru::console::Console;
use ctru::services::am::Title;
use ctru::services::fs::MediaType;
use ctru::services::hid::KeyPad;
use ctru::services::{am::Am, apt::Apt, gfx::Gfx, hid::Hid, soc::Soc};
use std::collections::HashMap;
use std::fs;
use std::fs::{File, create_dir_all};

use crate::archive::{ArchiveFileLeaf, CtruUserSaveArchive, walk_tree};
use crate::store::HttpStore;

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let _net = Soc::new()?;

    setup_sdmc()?;
    let sync_states = get_sync_states()?;
    let installed_titles = get_installed_titles(&am)?;
    let installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);

    println!("Active sync states: {:?}", installed_sync_states);
    println!("Press (A) to sync");
    println!("Press Start to exit");

    while apt.main_loop() {
        gfx.wait_for_vblank();

        hid.scan_input();

        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if hid.keys_down().contains(KeyPad::A) {
            let results = do_sync(&installed_sync_states);
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
    let mut s: Vec<SyncState> = Vec::new();

    for f in fs::read_dir("sdmc:/3ds/Cloudpoint")? {
        let f = f?;
        s.push(serde_json::from_reader(File::open(f.path())?)?);
    }

    Ok(s)
}

fn get_installed_titles(am: &Am) -> Result<Vec<Title>> {
    let titles = am.title_list(MediaType::Sd)?;

    Ok(titles)
}

fn get_installed_sync_states(
    sync_states: &[SyncState],
    installed_titles: &[Title],
) -> Vec<SyncState> {
    let ids = installed_titles.iter().map(|t| t.id()).collect::<Vec<_>>();
    sync_states
        .iter()
        .filter(|&s| ids.contains(&s.title_id))
        .cloned()
        .collect()
}

fn do_sync(active_sync_states: Vec<SyncState>) -> Result<Vec<Result<String>>> {
    let mut results = Vec::new();

    for mut s in active_sync_states {
        let list = cloudpoint_lib::version::VersionDirList::try_get(
            "http://192.168.1.163:8080",
            "dw",
            s.title_id,
        )?;

        let Some(e) = list.latest() else {
            results.push(Err(anyhow!(
                "failed to fetch version listing for {}",
                s.title_id
            )));
            continue;
        };

        //TODO! If the remote fp is bad, maybe just upload local anyway because it is broken on remote?
        s.set_remote_fp(e.fingerprint()?);

        let Ok(local_tree) = walk_tree(s.title_id) else {
            //TODO! Check it actually has an archive? Or just assume on error we pull it down if it is there?
            results.push(Err(anyhow!(
                "failed to build local version for title {}, run once to init and retry",
                s.title_id
            )));
            continue;
        };

        let local_ver = Version::new(&local_tree, HashMap::default(), 128, 512, 1024)?;

        s.set_local_fp(local_ver.fingerprint());

        match s.get_action() {
            SyncAction::Nothing => {
                results.push(Ok(format!("Nothing to do for title {}", s.title_id)))
            }
            SyncAction::Conflict => results.push(Ok(format!(
                "Conflict exists for title {}, cannot sync",
                s.title_id
            ))),
            SyncAction::Upload => {
                let mut store = HttpStore("http://192.168.1.163:8080".into());
                local_ver.copy_chunks(&local_tree, &mut store)?;
                // TODO! Add new version file, upload, update syncstate with fingerprints, profit!
            }
            SyncAction::Download => {}
        }

        let Ok(v) = e.get_version::<ArchiveFileLeaf>("http://192.168.1.163:8080", "dw", s.title_id)
        else {
            results.push(Err(anyhow!(
                "failed to fetch version file for {}",
                s.title_id
            )));
            continue;
        };
    }

    Ok(results)
}
