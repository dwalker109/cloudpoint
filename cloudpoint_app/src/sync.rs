use crate::{
    ctr_fs::CtrArchive,
    services::{CtrGfxServices, CtrSysServices},
    settings::SETTINGS,
    store::HttpStore,
    tree::{self, CtrArchiveLeaf},
};
use anyhow::Result;
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
use ctru::services::hid::KeyPad;
use std::{collections::HashMap, fs, rc::Rc};

pub fn run(
    services: &mut CtrSysServices,
    gfx_services: &CtrGfxServices,
    active_sync_states: &mut HashMap<(u64, CtrArchiveKind), SyncState>,
) -> Result<()> {
    let client = Rc::new(CurlHttpClient::new()?);

    for mut s in active_sync_states.values_mut() {
        log::info!("Starting sync for {:016x} {}", s.title_id, s.archive_kind);

        println!("\n{:016x} {}", s.title_id, s.archive_kind);

        let list = cloudpoint_lib::version::VersionDirList::try_get(
            &client,
            &SETTINGS.base_url,
            &SETTINGS.user_key,
            s.title_id,
            s.archive_kind,
        )?;
        s.remote_fp = list.latest().and_then(|e| e.fingerprint().ok());

        let archive = Rc::new(CtrArchive::open(s.title_id, s.archive_kind)?);

        let Ok(local_tree) = tree::from_archive(Rc::clone(&archive)) else {
            log::info!(
                "{} archive does not exist for title {:016x}",
                s.archive_kind,
                s.title_id
            );

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
                log::info!(
                    "No local or remote data for {:016x} {}",
                    s.title_id,
                    s.archive_kind
                );

                println!("Nothing to do, no local or remote data!");
            }
            SyncAction::NoChange => {
                log::info!(
                    "Local and remote data match for {:016x} {}",
                    s.title_id,
                    s.archive_kind
                );

                println!("Nothing to do, local and remote data match!");
            }
            SyncAction::Conflict => {
                log::info!(
                    "Changed on server and locally for {:016x} {}",
                    s.title_id,
                    s.archive_kind
                );

                println!("Changed on server and locally!");
                println!("DPAD UP to upload (local wins)");
                println!("DPAD DOWN to download (remote wins)");
                println!("DPAD LEFT or DPAD RIGHT to skip (come back later)");

                while services.apt.main_loop() {
                    gfx_services.gfx.wait_for_vblank();
                    services.hid.scan_input();

                    if services.hid.keys_down().contains(KeyPad::DPAD_UP) {
                        ul(&mut s, Rc::clone(&client), &local_ver, &local_tree)?;
                        break;
                    } else if services.hid.keys_down().contains(KeyPad::DPAD_DOWN) {
                        dl(
                            &mut s,
                            Rc::clone(&client),
                            Rc::clone(&archive),
                            &local_ver,
                            local_tree,
                        )?;
                        break;
                    } else if services
                        .hid
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

        log::info!("Sync completed for {:016x} {}", s.title_id, s.archive_kind);
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
    log::info!("Uploading {:016x} {}", s.title_id, s.archive_kind);

    let mut store = HttpStore::new(Rc::clone(&client), SETTINGS.base_url.clone());
    local_ver.copy_chunks(&local_tree, &mut store)?;

    VersionDirEntry::put_version(
        &client,
        &SETTINGS.base_url,
        &SETTINGS.user_key,
        s.title_id,
        s.archive_kind,
        &local_ver,
    )?;

    s.last_fp = Some(local_ver.fingerprint());

    write_db(s)?;

    println!("Uploaded!");

    Ok(())
}

fn dl(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    archive: Rc<CtrArchive>,
    local_ver: &Version<CtrArchiveLeaf>,
    local_tree: Tree<CtrArchiveLeaf>,
) -> Result<()> {
    log::info!("Downloading {:016x} {}", s.title_id, s.archive_kind);

    let Ok(remote_ver) = VersionDirEntry::get_version::<CtrArchiveLeaf>(
        &client,
        &SETTINGS.base_url,
        &SETTINGS.user_key,
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
    let store = HttpStore::new(Rc::clone(&client), SETTINGS.base_url.clone());
    let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

    while !u.is_terminal() {
        u.update_next()?;
    }

    archive.finalise()?;

    s.last_fp = Some(remote_ver.fingerprint());

    write_db(s)?;

    println!("Downloaded!");

    Ok(())
}

fn write_db(s: &SyncState) -> Result<()> {
    log::info!("Writing db for {:016x} {}", s.title_id, s.archive_kind);

    fs::write(
        format!(
            "sdmc:/3ds/Cloudpoint/db/{}.{}",
            s.product_code, s.archive_kind
        ),
        postcard::to_allocvec(&s)?,
    )?;

    Ok(())
}
