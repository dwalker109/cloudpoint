mod archive;
mod store;
mod config {
    pub const BASE_URL: &'static str = "http://192.168.1.45:8080";
    pub const USER_KEY: &'static str = "dw";
}

use anyhow::{Context, Result, anyhow};
use chunktree::{
    store::MemStore,
    tree::Tree,
    version::{Diff, Version, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    sync::{SyncAction, SyncState},
    version::VersionDirEntry,
};
use ctru::{
    console::Console,
    services::{
        am::{Am, Title},
        apt::Apt,
        fs::MediaType,
        gfx::Gfx,
        hid::{Hid, KeyPad},
        soc::Soc,
    },
};
use std::{
    collections::HashMap,
    fs::{self, File, create_dir_all},
};

use crate::archive::{ArchiveFileLeaf, walk_tree};
use crate::config::{BASE_URL, USER_KEY};
use crate::store::HttpStore;

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let mut _soc = Soc::new()?;

    // soc.redirect_to_3dslink(true, true)?;

    setup_sdmc()?;
    let sync_states = get_sync_states()?;
    let installed_titles = get_installed_titles(&am)?;
    let installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);

    println!("Available sync states: {:?}", sync_states.len());
    println!("Active sync states: {:?}", installed_sync_states.len());
    println!("Press (A) to sync");
    println!("Press Start to exit");

    while apt.main_loop() {
        gfx.wait_for_vblank();

        hid.scan_input();

        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if hid.keys_down().contains(KeyPad::A) {
            let res = do_sync(&apt, &mut hid, &gfx, installed_sync_states.clone());
            println!("Results: {:?}", res);
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

fn do_sync(apt: &Apt, hid: &mut Hid, gfx: &Gfx, active_sync_states: Vec<SyncState>) -> Result<()> {
    for mut s in active_sync_states {
        let list = cloudpoint_lib::version::VersionDirList::try_get(BASE_URL, "dw", s.title_id)?;
        s.remote_fp = list.latest().and_then(|e| e.fingerprint().ok());

        println!(
            "Using remote version {:0x} for title {:0x}",
            s.remote_fp.unwrap_or_default(),
            s.title_id
        );

        let Ok(local_tree) = walk_tree(s.title_id) else {
            //TODO! Check it actually has an archive? Or just assume on error we pull it down if it is there?
            println!(
                "Failed to get archive for title {:0x}, run once to init and retry",
                s.title_id
            );
            continue;
        };

        let local_ver = Version::new(&local_tree, HashMap::default(), 128, 512, 1024)?;
        s.local_fp = Some(local_ver.fingerprint());

        println!(
            "Using local version {:0x} for title {:0x}",
            s.local_fp.unwrap_or_default(),
            s.title_id
        );

        match s.get_action() {
            SyncAction::Nothing => {
                println!("Nothing to do for title {:0x}", s.title_id);
            }
            SyncAction::Conflict => {
                println!("Title {:0x} changed on server and locally!", s.title_id);
                println!("Press DPAD UP to upload (local wins)");
                println!("Press DPAD DOWN to download (remote wins)");
                println!("Press DPAD LEFT or DPAD RIGHT to skip (come back later)");

                while apt.main_loop() {
                    gfx.wait_for_vblank();
                    hid.scan_input();

                    if hid.keys_down().contains(KeyPad::DPAD_UP) {
                        ul(&mut s, &local_ver, &local_tree)?;
                        break;
                    } else if hid.keys_down().contains(KeyPad::DPAD_DOWN) {
                        dl(&mut s, &local_ver, local_tree)?;
                        break;
                    } else if hid
                        .keys_down()
                        .intersects(KeyPad::DPAD_LEFT | KeyPad::DPAD_RIGHT)
                    {
                        break;
                    }
                }
            }
            SyncAction::Upload => {
                ul(&mut s, &local_ver, &local_tree)?;
            }
            SyncAction::Download => {
                dl(&mut s, &local_ver, local_tree)?;
            }
        }
    }

    Ok(())
}

fn ul(
    s: &mut SyncState,
    local_ver: &Version<ArchiveFileLeaf>,
    local_tree: &Tree<ArchiveFileLeaf>,
) -> Result<()> {
    let mut store = HttpStore(BASE_URL.into());
    local_ver.copy_chunks(&local_tree, &mut store)?;
    VersionDirEntry::put_version(BASE_URL, USER_KEY, s.title_id, &local_ver)?;

    s.last_fp = Some(local_ver.fingerprint());
    fs::write(
        format!("sdmc:/3ds/Cloudpoint/db/{}", s.product_code),
        serde_json::to_string_pretty(&s)?,
    )
    .context("writing local db state")?;

    println!("Uploaded save for title {:0x}", s.title_id);

    Ok(())
}

fn dl(
    s: &mut SyncState,
    local_ver: &Version<ArchiveFileLeaf>,
    local_tree: Tree<ArchiveFileLeaf>,
) -> Result<()> {
    let Ok(remote_ver) = VersionDirEntry::get_version::<ArchiveFileLeaf>(
        BASE_URL,
        USER_KEY,
        s.title_id,
        s.remote_fp
            .expect("unreachable without a remote version available"),
    ) else {
        println!("failed to fetch version manifest for {:0x}", s.title_id);
        return Ok(());
    };

    let diff = Diff::new(&local_ver, &remote_ver);
    let cache = MemStore::default();
    let store = HttpStore(BASE_URL.into());
    let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

    while !u.is_terminal() {
        u.update_next()?;
    }

    s.last_fp = Some(remote_ver.fingerprint());
    fs::write(
        format!("sdmc:/3ds/Cloudpoint/db/{}", s.product_code),
        serde_json::to_string_pretty(&s)?,
    )
    .context("writing local db state")?;

    println!("Downloaded save for title {:0x}", s.title_id);

    Ok(())
}
