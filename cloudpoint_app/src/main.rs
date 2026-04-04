use crate::{ctr_fs::CtrArchive, settings::SETTINGS, store::HttpStore, tree::CtrArchiveLeaf};
use anyhow::{Context, Result};
use chunktree::{
    store::MemStore,
    tree::Tree,
    version::{Diff, Version, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    http::CurlHttpClient,
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
    fs::{self, create_dir_all, read_to_string},
    rc::Rc,
};

mod ctr_fs;
mod settings;
mod store;
mod tree;

fn main() -> Result<()> {
    flexi_logger::Logger::try_with_str("debug")?
        .log_to_file(flexi_logger::FileSpec::default().directory("sdmc:/3ds/Cloudpoint/logs"))
        .start()?;

    let am = Am::new()?;
    let apt = Apt::new()?;
    let mut hid = Hid::new()?;
    let gfx = Gfx::new()?;
    let _console = Console::new(gfx.top_screen.borrow_mut());
    let mut _soc = Soc::new()?;

    setup_sdmc()?;

    let installed_titles = get_installed_titles(&am)?;
    let mut sync_states = load_db_all(&installed_titles)?;
    append_autoadd(&installed_titles, &mut sync_states)?;

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

    while apt.main_loop() {
        gfx.wait_for_vblank();

        hid.scan_input();

        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        if hid.keys_down().contains(KeyPad::A) {
            let res = do_sync(&apt, &mut hid, &gfx, &mut sync_states);
            println!("Results: {:?}", res);
        }
    }

    Ok(())
}

fn setup_sdmc() -> Result<()> {
    let paths = [
        "sdmc:/3ds/Cloudpoint",
        "sdmc:/3ds/Cloudpoint/db",
        "sdmc:/3ds/Cloudpoint/logs",
    ];
    for p in paths {
        create_dir_all(p).with_context(|| format!("fatal: failed to create directory {p}"))?;
    }

    log::debug!("Created paths");

    Ok(())
}

fn append_autoadd(
    installed_titles: &[Title],
    sync_states: &mut HashMap<(u64, CtrArchiveKind), SyncState>,
) -> Result<()> {
    let titles = installed_titles
        .iter()
        .map(|t| (t.product_code().trim_end_matches('\0').to_string(), t.id()))
        .collect::<HashMap<_, _>>();

    for (product_code, archive_kind) in read_to_string(format!("sdmc:/3ds/Cloudpoint/autoadd.txt"))?
        .lines()
        .filter_map(|l| l.split_once(','))
        .filter_map(|(product_code, kind)| {
            CtrArchiveKind::try_from(kind)
                .ok()
                .and_then(|kind| Some((product_code.to_string(), kind)))
        })
    {
        if let Some(&title_id) = titles.get(&product_code)
            && !sync_states.contains_key(&(title_id, archive_kind))
        {
            let state = SyncState {
                title_id,
                product_code,
                archive_kind,
                last_fp: None,
                local_fp: None,
                remote_fp: None,
            };

            sync_states.insert((title_id, archive_kind), state);
        }
    }

    Ok(())
}

fn get_installed_titles<'a>(am: &'a Am) -> Result<Vec<Title<'a>>> {
    let titles = am.title_list(MediaType::Sd)?;

    Ok(titles)
}

fn load_db_all(installed_titles: &[Title]) -> Result<HashMap<(u64, CtrArchiveKind), SyncState>> {
    let ids = installed_titles.iter().map(|t| t.id()).collect::<Vec<_>>();

    let mut states: Vec<SyncState> = Vec::new();

    for f in fs::read_dir("sdmc:/3ds/Cloudpoint/db")? {
        let f = f?;
        if let Ok(s) = postcard::from_bytes(&fs::read(f.path())?) {
            states.push(s);
        }
    }

    Ok(states
        .into_iter()
        .filter(|s| ids.contains(&s.title_id))
        .map(|s| ((s.title_id, s.archive_kind), s))
        .collect())
}

fn write_db(s: &SyncState) -> Result<()> {
    fs::write(
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_kind
        ),
        postcard::to_allocvec(&s)?,
    )?;

    Ok(())
}

fn do_sync(
    apt: &Apt,
    hid: &mut Hid,
    gfx: &Gfx,
    active_sync_states: &mut HashMap<(u64, CtrArchiveKind), SyncState>,
) -> Result<()> {
    let client = Rc::new(CurlHttpClient::new()?);

    for mut s in active_sync_states.values_mut() {
        println!("\n{:016x} {}", s.title_id, s.archive_kind);
        let list = cloudpoint_lib::version::VersionDirList::try_get(
            &client,
            SETTINGS.base_url(),
            SETTINGS.user_key(),
            s.title_id,
            s.archive_kind,
        )?;
        s.remote_fp = list.latest().and_then(|e| e.fingerprint().ok());

        let archive = Rc::new(CtrArchive::open(s.title_id, s.archive_kind)?);

        let Ok(local_tree) = tree::from_archive(Rc::clone(&archive)) else {
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
            SyncAction::NoData => {
                println!("Nothing to do, no local or remote data!");
            }
            SyncAction::NoChange => {
                println!("Local and remote data match!");
                s.last_fp = s.local_fp;
                write_db(s)?;
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
                        ul(&mut s, Rc::clone(&client), &local_ver, &local_tree)?;
                        break;
                    } else if hid.keys_down().contains(KeyPad::DPAD_DOWN) {
                        dl(
                            &mut s,
                            Rc::clone(&client),
                            Rc::clone(&archive),
                            &local_ver,
                            local_tree,
                        )?;
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
                ul(&mut s, Rc::clone(&client), &local_ver, &local_tree)?;
            }
            SyncAction::Download => {
                dl(
                    &mut s,
                    Rc::clone(&client),
                    Rc::clone(&archive),
                    &local_ver,
                    local_tree,
                )?;
            }
        }
    }

    println!("\nDone!");

    Ok(())
}

fn ul(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: &Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let mut store = HttpStore::new(Rc::clone(&client), SETTINGS.base_url().into());
    local_ver.copy_chunks(&local_tree, &mut store)?;

    VersionDirEntry::put_version(
        &client,
        SETTINGS.base_url(),
        SETTINGS.user_key(),
        s.title_id,
        s.archive_kind,
        &local_ver,
    )?;

    s.last_fp = Some(local_ver.fingerprint());

    write_db(s)?;

    println!("Uploaded {}!", s.archive_kind);

    Ok(())
}

fn dl(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    archive: Rc<CtrArchive>,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: Tree<CtrArchiveLeaf>,
) -> Result<()> {
    let Ok(remote_ver) = VersionDirEntry::get_version::<CtrArchiveLeaf>(
        &client,
        SETTINGS.base_url(),
        SETTINGS.user_key(),
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
    let store = HttpStore::new(Rc::clone(&client), SETTINGS.base_url().into());
    let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

    while !u.is_terminal() {
        u.update_next()?;
    }

    archive.finalise()?;

    s.last_fp = Some(remote_ver.fingerprint());

    write_db(s)?;

    println!("Downloaded {}!", s.archive_kind);

    Ok(())
}
