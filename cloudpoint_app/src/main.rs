mod archive;
mod store;
mod config {
    pub const BASE_URL: &'static str = "http://192.168.1.45:8080";
    pub const USER_KEY: &'static str = "dw";
}

use anyhow::{Context, Result, anyhow};
use chunktree::store::MemStore;
use chunktree::version::updater::BlockingUpdater;
use chunktree::version::{Diff, Version};
use cloudpoint_lib::sync::{SyncAction, SyncState};
use cloudpoint_lib::version::VersionDirEntry;
use ctru::console::Console;
use ctru::services::am::Title;
use ctru::services::fs::MediaType;
use ctru::services::hid::KeyPad;
use ctru::services::{am::Am, apt::Apt, gfx::Gfx, hid::Hid, soc::Soc};
use std::collections::HashMap;
use std::fs;
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::net::TcpStream;

use crate::archive::{ArchiveFileLeaf, CtruUserSaveArchive, walk_tree};
use crate::config::{BASE_URL, USER_KEY};
use crate::store::HttpStore;

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let mut soc = Soc::new()?;

    // soc.redirect_to_3dslink(true, true)?;

    setup_sdmc()?;
    let sync_states = get_sync_states()?;
    let installed_titles = get_installed_titles(&am)?;
    let installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);

    println!("Available sync states: {:?}", sync_states);
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
            let results = do_sync(installed_sync_states.clone());
            println!("Results: {:?}", results);
        }
    }

    Ok(())
}

fn setup_sdmc() -> Result<()> {
    let paths = ["sdmc:/3ds/Cloudpoint", "sdmc:/3ds/Cloudpoint/db"];
    for p in paths {
        create_dir_all(p).with_context(|| format!("fatal: failed to create directory {p}"))?;
    }

    Ok(())
}

fn get_sync_states() -> Result<Vec<SyncState>> {
    let mut states: Vec<SyncState> = Vec::new();

    for f in fs::read_dir("sdmc:/3ds/Cloudpoint/db")? {
        let f = f?;
        if let Ok(s) = serde_json::from_reader(File::open(f.path())?) {
            states.push(s);
        }
    }

    Ok(states)
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
        let list = cloudpoint_lib::version::VersionDirList::try_get(BASE_URL, "dw", s.title_id)?;
        let remote_version_meta = list.latest();

        if let Some(existing) = remote_version_meta {
            match existing.fingerprint() {
                Ok(f) => s.fingerprint_remote_curr = Some(f),
                Err(e) => {
                    results.push(Err(anyhow!(
                        "latest version listing for {} has bad fingerprint: {}",
                        s.title_id,
                        e
                    )));
                    continue;
                }
            }
        } else {
            s.fingerprint_remote_curr = None;
        }

        let Ok(local_tree) = walk_tree(s.title_id) else {
            //TODO! Check it actually has an archive? Or just assume on error we pull it down if it is there?
            results.push(Err(anyhow!(
                "failed to build local version for title {}, run once to init and retry",
                s.title_id
            )));
            continue;
        };

        let local_ver = Version::new(&local_tree, HashMap::default(), 128, 512, 1024)?;
        s.fingerprint_local_curr = Some(local_ver.fingerprint());

        println!("");
        dbg!(remote_version_meta);
        dbg!(&s);

        match s.get_action() {
            SyncAction::Nothing => {
                results.push(Ok(format!("Nothing to do for title {}", s.title_id)))
            }
            SyncAction::Conflict => results.push(Ok(format!(
                "Conflict exists for title {}, cannot sync",
                s.title_id
            ))),
            SyncAction::Upload => {
                let mut store = HttpStore(BASE_URL.into());
                local_ver.copy_chunks(&local_tree, &mut store)?;
                VersionDirEntry::put_version(BASE_URL, USER_KEY, s.title_id, &local_ver)?;

                s.fingerprint_local_last = Some(local_ver.fingerprint());
                s.fingerprint_remote_last = s.fingerprint_local_last;
                fs::write(
                    format!("sdmc:/3ds/Cloudpoint/db/{}", s.product_code),
                    serde_json::to_string_pretty(&s)?,
                )
                .context("writing local db state")?;

                results.push(Ok(format!("Uploaded save for title {}", s.title_id)));
            }
            SyncAction::Download => {
                let Ok(remote_ver) = remote_version_meta
                    .expect("only reachable when this is Some<_> ")
                    .get_version::<ArchiveFileLeaf>(BASE_URL, USER_KEY, s.title_id)
                else {
                    results.push(Err(anyhow!(
                        "failed to fetch version manifest for {}",
                        s.title_id
                    )));
                    continue;
                };

                let diff = Diff::new(&local_ver, &remote_ver);
                let cache = MemStore::default();
                let store = HttpStore(BASE_URL.into());
                let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

                while !u.is_terminal() {
                    //TODO! Move archive commit logic out of Drop impl so we can rollback...
                    u.update_next()?;
                }

                s.fingerprint_remote_last = Some(remote_ver.fingerprint());
                s.fingerprint_local_last = s.fingerprint_remote_last;
                fs::write(
                    format!("sdmc:/3ds/Cloudpoint/db/{}", s.product_code),
                    serde_json::to_string_pretty(&s)?,
                )
                .context("writing local db state")?;

                results.push(Ok(format!("Downloaded save for title {}", s.title_id)));
            }
        }
    }

    Ok(results)
}
