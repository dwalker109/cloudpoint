mod ctr_archive;
mod ffi;
mod store;
mod config {
    pub const BASE_URL: &'static str = "http://192.168.1.45:8080";
    pub const USER_KEY: &'static str = "dw";
}

use anyhow::{Context, Result, bail};
use chunktree::{
    store::MemStore,
    tree::Tree,
    version::{Diff, Version, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    sync::{CtrArchiveKind, SyncAction, SyncState},
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
    fs::{self, File, create_dir_all, read_to_string},
};

use crate::config::{BASE_URL, USER_KEY};
use crate::ctr_archive::{CtrArchiveLeaf, walk_tree};
use crate::store::HttpStore;

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let mut _soc = Soc::new()?;

    // soc.redirect_to_3dslink(true, true)?;

    let installed_titles = get_installed_titles(&am)?;

    setup_sdmc()?;
    autoadd(&installed_titles)?;

    let mut sync_states = get_sync_states()?;
    let mut installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);

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

            sync_states = get_sync_states()?;
            installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);
        }
    }

    Ok(())
}

fn setup_sdmc() -> Result<()> {
    let paths = [
        "sdmc:/3ds/Cloudpoint",
        "sdmc:/3ds/Cloudpoint/db",
        "sdmc:/3ds/Cloudpoint/autoadd",
    ];
    for p in paths {
        create_dir_all(p).with_context(|| format!("fatal: failed to create directory {p}"))?;
    }

    Ok(())
}

fn autoadd(installed_titles: &[Title]) -> Result<()> {
    for (add_code, mode) in read_to_string(format!("sdmc:/3ds/Cloudpoint/autoadd.txt"))
        .unwrap()
        .lines()
        .map(|l| l.split_once(',').unwrap())
    {
        if let Some((title_id, product_code)) = installed_titles
            .iter()
            .map(|t| (t.id(), t.product_code().trim_end_matches('\0').to_string()))
            .find(|t| t.1 == add_code)
        {
            let state = SyncState {
                title_id,
                product_code,
                archive_mode: match mode {
                    "savedata" => CtrArchiveKind::Savedata,
                    "extdata" => CtrArchiveKind::Extdata,
                    _ => bail!("there is a malformed entry in autoadd.txt ({})", add_code),
                },
                last_fp: None,
                local_fp: None,
                remote_fp: None,
            };

            let path = format!(
                "sdmc:/3ds/Cloudpoint/db/{}.{}",
                state.product_code, state.archive_mode
            );
            if !fs::exists(&path)? {
                fs::write(path, serde_json::to_string_pretty(&state)?)?;
            }
        }
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

fn get_installed_titles<'a>(am: &'a Am) -> Result<Vec<Title<'a>>> {
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
        let list = cloudpoint_lib::version::VersionDirList::try_get(
            BASE_URL,
            "dw",
            s.title_id,
            s.archive_mode,
        )?;
        s.remote_fp = list.latest().and_then(|e| e.fingerprint().ok());

        println!(
            "Using remote version {:016x} for title {:016x}",
            s.remote_fp.unwrap_or_default(),
            s.title_id
        );

        let Ok(local_tree) = walk_tree(s.title_id, s.archive_mode) else {
            //TODO! Check it actually has an archive? Or just assume on error we pull it down if it is there?
            println!(
                "Failed to get {:?} archive for title {:016x}, run once to init and retry",
                s.archive_mode, s.title_id
            );
            continue;
        };

        let local_ver = Version::new(&local_tree, HashMap::default(), 128_000, 512_000, 1024_000)?;
        s.local_fp = Some(local_ver.fingerprint());

        println!(
            "Using local version {:016x} for title {:016x}",
            s.local_fp.unwrap_or_default(),
            s.title_id
        );

        match s.get_action() {
            SyncAction::Nothing => {
                println!("Nothing to do for title {:016x}", s.title_id);
            }
            SyncAction::Conflict => {
                println!("Title {:016x} changed on server and locally!", s.title_id);
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
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: &Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let mut store = HttpStore(BASE_URL.into());
    local_ver.copy_chunks(&local_tree, &mut store)?;
    VersionDirEntry::put_version(BASE_URL, USER_KEY, s.title_id, s.archive_mode, &local_ver)?;

    s.last_fp = Some(local_ver.fingerprint());
    fs::write(
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_mode
        ),
        serde_json::to_string_pretty(&s)?,
    )
    .context("writing local db state")?;

    println!("Uploaded save for title {:016x}", s.title_id);

    Ok(())
}

fn dl(
    s: &mut SyncState,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let Ok(remote_ver) = VersionDirEntry::get_version::<CtrArchiveLeaf>(
        BASE_URL,
        USER_KEY,
        s.title_id,
        s.archive_mode,
        s.remote_fp
            .expect("unreachable without a remote version available"),
    ) else {
        println!("failed to fetch version manifest for {:016x}", s.title_id);
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
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_mode
        ),
        serde_json::to_string_pretty(&s)?,
    )
    .context("writing local db state")?;

    println!("Downloaded save for title {:016x}", s.title_id);

    Ok(())
}
