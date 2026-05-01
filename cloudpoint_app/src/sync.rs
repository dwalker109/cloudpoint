use crate::{
    config::{AppPath, BackupTarget, USER_KEY, USER_SETTINGS},
    ctr_fs::CtrArchive,
    db::StateDb,
    services::{CtrGfxServices, CtrSysServices},
    tree::{self, CtrArchiveLeaf},
};
use anyhow::Result;
use chunktree::{
    store::MemStore,
    tree::{Leaf, Tree},
    version::{ChunkStrategy, Concurrency, Diff, Version, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    ctr::{CtrMeta, SmdhLanguage},
    http::CurlHttpClient,
    store::HttpStore,
    sync::{SyncAction, SyncState},
    version::VersionDirEntry,
};
use ctru::services::hid::KeyPad;
use std::{
    fs::{self, File},
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
};

pub fn run(
    services: &mut CtrSysServices,
    gfx_services: &CtrGfxServices,
    state_db: &mut StateDb,
) -> Result<()> {
    let client = Rc::new(CurlHttpClient::new()?);

    for mut s in state_db.states_mut() {
        let smdh = CtrArchive::smdh(s.title_id, s.archive_kind)?;

        log::info!(
            "Starting sync for {} ({:016x}) {}",
            smdh.title_short(SmdhLanguage::English),
            s.title_id,
            s.archive_kind
        );

        println!(
            "\n{} ({})",
            smdh.title_short(SmdhLanguage::English),
            smdh.title_publisher(SmdhLanguage::English)
        );
        println!("{:016x} {}", s.title_id, s.archive_kind);

        let list = cloudpoint_lib::version::VersionDirList::try_get(
            &client,
            &USER_SETTINGS.base_url,
            &USER_KEY,
            s.title_id,
            s.archive_kind,
        )?;

        let remote_ver = list.latest();
        s.remote_fp = remote_ver.and_then(|e| e.fingerprint().ok());

        let local_meta = CtrArchive::meta(s.title_id)?;
        let local_archive = Rc::new(CtrArchive::open(s.title_id, s.archive_kind)?);

        let Ok(local_tree) = tree::from_archive(Rc::clone(&local_archive)) else {
            log::info!(
                "{} archive does not exist for title {:016x}",
                s.archive_kind,
                s.title_id
            );

            continue;
        };

        let local_ver = Version::new(
            &local_tree,
            local_meta,
            ChunkStrategy::FixedSize(256 * 1024),
            Concurrency::Serial,
        )?;
        s.local_fp = Some(local_ver.fingerprint());

        println!("Local \x1b[12C{:032x}", s.local_fp.unwrap_or_default());
        println!("Remote\x1b[12C{:032x}", s.remote_fp.unwrap_or_default());

        match s.get_action() {
            SyncAction::NoData => {
                log::info!(
                    "no local or remote data for {:016x} {}",
                    s.title_id,
                    s.archive_kind
                );

                println!("Nothing to do, no local or remote data!");
            }
            SyncAction::NoChange => {
                log::info!(
                    "local and remote data match for {:016x} {}",
                    s.title_id,
                    s.archive_kind
                );

                println!("Nothing to do, local and remote data match!");
            }
            SyncAction::Conflict => {
                log::info!(
                    "changed on server and locally for {:016x} {}",
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

        log::info!("sync completed for {:016x} {}", s.title_id, s.archive_kind);
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
    println!("Uploading...");

    let mut store = HttpStore::new(
        Rc::clone(&client),
        USER_SETTINGS.base_url.clone(),
        USER_KEY.clone(),
    );
    local_ver.copy_chunks(&local_tree, &mut store)?;

    VersionDirEntry::put_version(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        s.title_id,
        s.archive_kind,
        &local_ver,
    )?;

    s.last_fp = Some(local_ver.fingerprint());

    s.save(AppPath::Db)?;

    println!("Done!");

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
    println!("Downloading...");

    let Ok(remote_ver) = VersionDirEntry::get_version::<CtrArchiveLeaf, CtrMeta>(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
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

    if USER_SETTINGS.backup {
        backup(&local_tree, &s)?;
    }

    let diff = Diff::new(&local_ver, &remote_ver);
    let cache = MemStore::default();
    let store = HttpStore::new(
        Rc::clone(&client),
        USER_SETTINGS.base_url.clone(),
        USER_KEY.clone(),
    );
    let mut u = BlockingUpdater::start(diff, local_tree, cache, store)?;

    while !u.is_terminal() {
        u.update_next()?;
    }

    if u.progress().is_err() {
        log::info!(
            "an error occurred while downloading the version: {:?}",
            u.progress()
        );

        println!("Something went wrong downloading the remote version",);
    }

    archive.finalise()?;

    s.last_fp = Some(remote_ver.fingerprint());

    s.save(AppPath::Db)?;

    println!("Done!");

    Ok(())
}

fn backup(local_tree: &Tree<CtrArchiveLeaf>, sync_state: &SyncState) -> Result<()> {
    let root_dir = match USER_SETTINGS.backup_target {
        BackupTarget::Cloudpoint => AppPath::Backup.join(format!(
            "{:#016X} {}/{}/{}",
            sync_state.title_id,
            sync_state.fs_safe_name,
            sync_state.archive_kind,
            chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        )),
        BackupTarget::Checkpoint => AppPath::Checkpoint.join(format!(
            "{}/{:#07X} {}/{} (Cloudpoint)",
            match sync_state.archive_kind {
                cloudpoint_lib::ctr::CtrArchiveKind::Savedata => "saves",
                cloudpoint_lib::ctr::CtrArchiveKind::Extdata => "extdata",
            },
            (sync_state.title_id as u32) >> 8,
            sync_state.fs_safe_name,
            chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        )),
    };

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
