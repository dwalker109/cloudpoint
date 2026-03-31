use crate::ctr_fs::CtrArchiveLeaf;
use crate::store::HttpStore;
use crate::{
    config::{BASE_URL, USER_KEY},
    ctr_fs::CtrArchive,
};
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
    collections::{HashMap, HashSet},
    fs::{self, File, create_dir_all, read_to_string},
    sync::Arc,
};

mod ctr_fs;
mod store;
mod config {
    pub const BASE_URL: &'static str = "http://192.168.1.45:8080";
    pub const USER_KEY: &'static str = "dw";
}

fn main() -> Result<()> {
    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let mut _soc = Soc::new()?;

    let installed_titles = get_installed_titles(&am)?;

    setup_sdmc()?;
    autoadd(&installed_titles)?;

    let mut sync_states = get_sync_states()?;
    let mut installed_sync_states = get_installed_sync_states(&sync_states, &installed_titles);

    println!("\x1b[20CCloudpoint\n");
    println!(
        "Ready to sync {} states across {} titles",
        installed_sync_states
            .iter()
            .map(|s| s.title_id)
            .collect::<HashSet<_>>()
            .len(),
        installed_sync_states.len()
    );
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
    let titles = installed_titles
        .iter()
        .map(|t| (t.product_code().trim_end_matches('\0').to_string(), t.id()))
        .collect::<HashMap<_, _>>();

    for (product_code, kind) in read_to_string(format!("sdmc:/3ds/Cloudpoint/autoadd.txt"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.split_once(',').unwrap_or_default())
    {
        if let Some(&title_id) = titles.get(product_code) {
            let state = SyncState {
                title_id,
                product_code: product_code.to_string(),
                archive_kind: match kind {
                    "save" => CtrArchiveKind::Savedata,
                    "extdata" => CtrArchiveKind::Extdata,
                    _ => bail!(
                        "there is a malformed entry in autoadd.txt ({})",
                        product_code
                    ),
                },
                last_fp: None,
                local_fp: None,
                remote_fp: None,
            };

            let path = format!(
                "sdmc:/3ds/Cloudpoint/db/{}.{}",
                state.product_code, state.archive_kind
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
        println!("\n{:016x} {}", s.title_id, s.archive_kind);
        let list = cloudpoint_lib::version::VersionDirList::try_get(
            BASE_URL,
            "dw",
            s.title_id,
            s.archive_kind,
        )?;
        s.remote_fp = list.latest().and_then(|e| e.fingerprint().ok());

        let archive = Arc::new(CtrArchive::open(s.title_id, s.archive_kind)?);

        let Ok(local_tree) = ctr_fs::walk_tree(Arc::clone(&archive)) else {
            println!(
                "Cannot open {:?} archive for title {:016x}, run once to init and retry",
                s.archive_kind, s.title_id
            );

            continue;
        };

        let local_ver = Version::new(&local_tree, HashMap::default(), 128_000, 512_000, 1024_000)?;
        s.local_fp = Some(local_ver.fingerprint());

        print!("Local {:016x}", s.local_fp.unwrap_or_default());
        println!("\x1b[5C{:016x} Remote", s.remote_fp.unwrap_or_default());

        match s.get_action() {
            SyncAction::Nothing => {
                println!("Nothing to do!");
            }
            SyncAction::Conflict => {
                println!("Changed on server and locally!");
                println!("DPAD UP to upload (local wins)");
                println!("DPAD DOWN to download (remote wins)");
                println!("DPAD LEFT or DPAD RIGHT to skip (come back later)");

                while apt.main_loop() {
                    gfx.wait_for_vblank();
                    hid.scan_input();

                    if hid.keys_down().contains(KeyPad::DPAD_UP) {
                        ul(&mut s, &local_ver, &local_tree)?;
                        break;
                    } else if hid.keys_down().contains(KeyPad::DPAD_DOWN) {
                        dl(&mut s, Arc::clone(&archive), &local_ver, local_tree)?;
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
                dl(&mut s, Arc::clone(&archive), &local_ver, local_tree)?;
            }
        }
    }

    println!("\nDone!");

    Ok(())
}

fn ul(
    s: &mut SyncState,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: &Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let mut store = HttpStore(BASE_URL.into());
    local_ver.copy_chunks(&local_tree, &mut store)?;
    VersionDirEntry::put_version(BASE_URL, USER_KEY, s.title_id, s.archive_kind, &local_ver)?;

    s.last_fp = Some(local_ver.fingerprint());
    fs::write(
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_kind
        ),
        serde_json::to_string_pretty(&s)?,
    )?;

    println!("Uploaded {}!", s.archive_kind);

    Ok(())
}

fn dl(
    s: &mut SyncState,
    archive: Arc<CtrArchive>,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let Ok(remote_ver) = VersionDirEntry::get_version::<CtrArchiveLeaf>(
        BASE_URL,
        USER_KEY,
        s.title_id,
        s.archive_kind,
        s.remote_fp
            .expect("unreachable without a remote version available"),
    ) else {
        println!("Failed to fetch version manifest :(");

        return Ok(());
    };

    let diff = Diff::new(&local_ver, &remote_ver);
    let cache = MemStore::default();
    let store = HttpStore(BASE_URL.into());
    let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

    while !u.is_terminal() {
        u.update_next()?;
    }

    archive.finalise()?;

    s.last_fp = Some(remote_ver.fingerprint());
    fs::write(
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_kind
        ),
        serde_json::to_string_pretty(&s)?,
    )?;

    println!("Downloaded {}!", s.archive_kind);

    Ok(())
}
