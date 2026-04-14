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
    tree::{Leaf, Tree},
    version::{Diff, Version, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    http::CurlHttpClient,
    sync::{CtrArchiveKind, SyncAction, SyncState},
    version::{CtrMeta, VersionDirEntry},
};
use ctru::services::hid::KeyPad;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
};

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

        let remote_ver = list.latest();
        s.remote_fp = remote_ver.and_then(|e| e.fingerprint().ok());

        let local_meta = CtrArchive::meta(s.title_id, s.archive_kind)?;
        match local_meta {
            CtrMeta::Unavailable => {
                log::info!("title {:016X} is not available on this system", s.title_id);
                println!(
                    "Title {:016X} does not seem to be installed, so its data cannot be synced yet",
                    s.title_id
                );
            }
            CtrMeta::NotInitialized { title_version } => {
                match s.get_action() {
                    SyncAction::NoData => {
                        log::info!(
                            "No local or remote data for {:016x} {}",
                            s.title_id,
                            s.archive_kind
                        );

                        println!("Nothing to do, no local or remote data!");
                    }
                    SyncAction::NoChange | SyncAction::Conflict | SyncAction::Upload => {
                        unreachable!()
                    }
                    SyncAction::Download => {
                        log::info!(
                            "initialising {} for title {:016X}",
                            s.archive_kind,
                            s.title_id
                        );

                        println!(
                            "Archive {} for title {:016X} is being initialised",
                            s.archive_kind, s.title_id
                        );

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

                        let smdh = match s.archive_kind {
                            CtrArchiveKind::Savedata => None,
                            CtrArchiveKind::Extdata => CtrMeta::get_smdh(
                                &client,
                                &SETTINGS.base_url,
                                &SETTINGS.user_key,
                                s.title_id,
                                s.archive_kind,
                            )
                            .ok(),
                        };

                        CtrArchive::format_new(
                            s.title_id,
                            s.archive_kind,
                            remote_ver.meta(),
                            smdh,
                        )?;

                        println!("Initialised, now try again");
                    }
                }

                log::info!(
                    "initialising {} for title {:016X}",
                    s.archive_kind,
                    s.title_id
                );

                println!(
                    "Archive {} for title {:016X} is being initialised",
                    s.archive_kind, s.title_id
                );
            }
            CtrMeta::Initialized { .. } => {
                let local_archive = Rc::new(CtrArchive::open(s.title_id, s.archive_kind)?);

                let Ok(local_tree) = tree::from_archive(Rc::clone(&local_archive)) else {
                    log::info!(
                        "{} archive does not exist for title {:016x}",
                        s.archive_kind,
                        s.title_id
                    );

                    continue;
                };

                let local_ver = Version::new(&local_tree, local_meta, 128_000, 512_000, 1024_000)?;
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
                                    Rc::clone(&local_archive),
                                    &local_meta,
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
                            Rc::clone(&local_archive),
                            &local_meta,
                            &local_ver,
                            local_tree,
                        )?;
                    }
                }

                log::info!("Sync completed for {:016x} {}", s.title_id, s.archive_kind);
            }
        };
    }

    println!("\nDone!");

    Ok(())
}

fn ul(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
    local_tree: &Tree<CtrArchiveLeaf>,
) -> Result<()> {
    log::info!("Uploading {:016x} {}", s.title_id, s.archive_kind);

    if s.archive_kind == CtrArchiveKind::Extdata {
        let smdh = CtrArchive::smdh(s.title_id, s.archive_kind)?;

        CtrMeta::put_smdh(
            &client,
            &SETTINGS.base_url,
            &SETTINGS.user_key,
            s.title_id,
            s.archive_kind,
            &smdh,
        )?;
    }

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
    local_meta: &CtrMeta,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
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

    if local_meta.title_version() != remote_ver.meta().title_version() {
        log::info!(
            "title versions do not match, cannot sync: local={:?} remote={:?}",
            local_meta.title_version(),
            remote_ver.meta().title_version()
        );

        println!(
            "Title version mismatch: local={:?} remote={:?} (ensure you are running the latest version on all consoles and try again)",
            local_meta.title_version(),
            remote_ver.meta().title_version()
        );

        return Ok(());
    }

    if SETTINGS.backup {
        backup(&local_tree, &s)?;
    }

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

fn backup(local_tree: &Tree<CtrArchiveLeaf>, sync_state: &SyncState) -> Result<()> {
    let root_dir = PathBuf::from(format!(
        "sdmc:/3ds/Cloudpoint/backups/{:016X}/{:016X}",
        sync_state.title_id,
        sync_state
            .local_fp
            .expect("Unreachable without local fingerprint")
    ));

    log::info!("Backing up to {:?}", root_dir);

    for leaf in local_tree.leaves() {
        let dst_path: PathBuf = root_dir.join(
            leaf.path()
                .components()
                .filter(|c| matches!(c, std::path::Component::Normal(_)))
                .collect::<PathBuf>(),
        );

        fs::create_dir_all(dst_path.parent().expect("file has parent directory"))?;
        let mut writer = BufWriter::new(File::create(dst_path)?);
        io::copy(&mut leaf.data()?, &mut writer)?;
    }

    log::info!("Backup complete");

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
