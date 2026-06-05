use crate::{
    app::{OpenModalMsg, SyncProgress, UiMsg},
    config::{AppPath, USER_KEY, USER_SETTINGS},
    ctr_fs::CtrArchive,
    ctr_ndmu::KeepAwake,
    ctr_title::meta,
    db::{InstallHistoryDb, InstallStatus},
    tree::{self, CtrArchiveLeaf},
};
use anyhow::{Result, bail};
use chrono::Utc;
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
    utils::ellipsis,
    version::{RemoteVersionMeta, get_version, put_version},
};
use ctru::services::ac::Ac;
use std::{
    fs::{self, File},
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
};

pub enum ConflictWinner {
    Local,
    Remote,
    Undecided,
}

pub fn run<'a>(
    states: impl Iterator<Item = &'a mut SyncState>,
    shutdown_rx: &Receiver<()>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
    client: &Rc<CurlHttpClient>,
    install_history_db: &mut InstallHistoryDb,
) -> Result<()> {
    log::info!("starting sync");

    let _keep_awake = KeepAwake::new();
    let mut sync_progress = SyncProgress::new(ui_tx);

    let states = states.collect::<Vec<_>>();
    let total = states.len();

    for (i, sync_state) in states.into_iter().enumerate() {
        if let Err(mpsc::TryRecvError::Disconnected) = shutdown_rx.try_recv() {
            log::info!("aborting mid sync due to app shutdown");
            return Ok(());
        }

        match run_one(
            sync_state,
            &mut sync_progress,
            &modal_tx,
            &client,
            install_history_db,
        ) {
            Ok(_) => sync_progress.progress((i + 1) * 100 / total),
            Err(e) => {
                log::error!("failed mid sync: {e}");
                return Err(e);
            }
        };
    }

    log::info!("completed sync");

    Ok(())
}

fn run_one(
    sync_state: &mut SyncState,
    sync_progress: &mut SyncProgress,
    modal_tx: &Sender<OpenModalMsg>,
    client: &Rc<CurlHttpClient>,
    install_history_db: &mut InstallHistoryDb,
) -> Result<()> {
    log::info!("Starting sync of {}", sync_state.sync_item);

    if Ac::new()?.wait_internet_connection().is_err() {
        log::warn!("internet down");
        bail!("No internet connection is available - please ensure you are online and try again");
    }

    let Ok(smdh) = CtrArchive::smdh(sync_state.sync_item) else {
        log::warn!("{} cannot be read, cannot sync", sync_state.sync_item,);

        bail!(
            "{} could not be opened; was the title ({}) deleted?",
            sync_state.sync_item,
            sync_state.title_short
        );
    };

    for title_id in &sync_state.via_title_ids {
        match install_history_db.check(*title_id) {
            InstallStatus::Updated => {
                log::info!("via_title_id {title_id:016X}: updated tmd mtime, reset sync meta");
                sync_state.synced_at = None;
                sync_state.synced_fingerprint = None;
                install_history_db.touch(*title_id);
            }
            InstallStatus::Unchanged => {
                log::debug!("via_title_id {title_id:016X}: unchanged tmd mtime, leave sync meta");
            }
            InstallStatus::Unknown => {
                log::warn!(
                    "via_title_id {title_id:016X}: unknown tmd mtime, probably shared extdata, leave sync meta"
                );
            }
        }
    }

    if sync_state.via_user_key != *USER_KEY {
        log::info!("user.key has changed, adopting (new sync dialog is normal)");

        sync_state.synced_at = None;
        sync_state.synced_fingerprint = None;
        sync_state.via_user_key = *USER_KEY;
    }

    let title_label = ellipsis(
        &format!(
            "{} ({})",
            smdh.title_short(SmdhLanguage::English),
            smdh.title_publisher(SmdhLanguage::English)
        ),
        40,
    );

    sync_progress.label(&title_label).message("Checking").send();

    let remote_ver = RemoteVersionMeta::latest(
        client,
        &USER_SETTINGS.base_url,
        *USER_KEY,
        sync_state.sync_item,
    );
    let remote_fingerprint = remote_ver.as_ref().map(|meta| meta.fingerprint()).ok();

    let local_meta = meta(sync_state.sync_item)?;
    let local_archive = Rc::new(CtrArchive::open(sync_state.sync_item)?);
    let local_tree = tree::from_archive(Rc::clone(&local_archive))?;
    let local_ver = Version::new(
        &local_tree,
        local_meta,
        ChunkStrategy::FixedSize(256 * 1024),
        Concurrency::Serial,
    )?;
    let local_fingerprint = Some(local_ver.fingerprint());

    log::debug!("Local: {:032x}", local_fingerprint.unwrap_or_default());
    log::debug!("Remote: {:032x}", remote_fingerprint.unwrap_or_default());

    match sync_state.get_action(local_fingerprint, remote_fingerprint) {
        SyncAction::NoChange | SyncAction::NoChangeOnInit => {
            log::info!("local and remote data match for {}", sync_state.sync_item,);

            if sync_state.synced_fingerprint.is_none() {
                sync_state.synced_fingerprint = local_fingerprint;
            }

            sync_state.synced_at = Some(Utc::now());
        }
        SyncAction::Conflict | SyncAction::ConflictOnInit => {
            log::info!("changed on server and locally for {}", sync_state.sync_item,);

            let (reply_tx, reply_rx) = oneshot::channel::<ConflictWinner>();

            modal_tx
                .send(OpenModalMsg::ResolveConflict {
                    title_label: title_label.clone(),
                    title_local_time: sync_state.synced_at,
                    title_remote_time: remote_ver.as_ref().map(|v| v.created_at).ok(),
                    reply_tx,
                })
                .ok();

            match reply_rx.recv()? {
                ConflictWinner::Local => {
                    ul(
                        sync_state,
                        Rc::clone(&client),
                        sync_progress,
                        &local_ver,
                        &local_tree,
                        local_fingerprint,
                    )?;
                }
                ConflictWinner::Remote => {
                    dl(
                        sync_state,
                        Rc::clone(&client),
                        sync_progress,
                        Rc::clone(&local_archive),
                        &local_meta,
                        &local_ver,
                        local_tree,
                        remote_fingerprint,
                    )?;
                }
                ConflictWinner::Undecided => {}
            };
        }
        SyncAction::Upload => {
            ul(
                sync_state,
                Rc::clone(&client),
                sync_progress,
                &local_ver,
                &local_tree,
                local_fingerprint,
            )?;
        }
        SyncAction::Download => {
            dl(
                sync_state,
                Rc::clone(&client),
                sync_progress,
                Rc::clone(&local_archive),
                &local_meta,
                &local_ver,
                local_tree,
                remote_fingerprint,
            )?;
        }
    }

    log::info!("sync completed for {}", sync_state.sync_item);

    Ok(())
}

fn ul(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    sync_progress: &mut SyncProgress,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
    local_tree: &Tree<CtrArchiveLeaf>,
    local_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("uploading {}", s.sync_item);

    sync_progress.message("Uploading").send();

    let mut store = HttpStore::new(
        Rc::clone(&client),
        USER_SETTINGS.base_url.clone(),
        USER_KEY.clone(),
    );
    local_ver.copy_chunks(&local_tree, &mut store)?;

    put_version(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        s.sync_item,
        &local_ver,
    )?;

    s.synced_fingerprint = local_fingerprint;
    s.synced_at = Some(Utc::now());

    Ok(())
}

fn dl(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    sync_progress: &mut SyncProgress,
    archive: Rc<CtrArchive>,
    local_meta: &CtrMeta,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
    local_tree: Tree<CtrArchiveLeaf>,
    remote_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("downloading {}", s.sync_item);

    let remote_ver = get_version::<CtrArchiveLeaf, CtrMeta>(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        s.sync_item,
        remote_fingerprint.expect("remote_fingerprint should be Some<u128> to init a download"),
    )?;

    if local_meta.required_version() != remote_ver.meta().required_version() {
        log::info!(
            "title versions do not match, cannot sync: local={:?} remote={:?}",
            local_meta.required_version(),
            remote_ver.meta().required_version()
        );

        bail!(
            "Title version mismatch: local={:?} remote={:?} (ensure you are running the latest version on all consoles and try again)",
            local_meta.required_version(),
            remote_ver.meta().required_version()
        );
    }

    if USER_SETTINGS.backup {
        sync_progress.message("Backing up existing data").send();
        backup(&local_tree, &s)?;
    }

    sync_progress.message("Downloading").send();

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
        log::error!(
            "error occurred while downloading version: {:?}",
            u.progress()
        );

        bail!("Something went wrong downloading the remote version");
    }

    archive.finalise()?;

    s.synced_fingerprint = remote_fingerprint;
    s.synced_at = Some(Utc::now());

    Ok(())
}

fn backup(local_tree: &Tree<CtrArchiveLeaf>, sync_state: &SyncState) -> Result<()> {
    let root_dir = AppPath::Backup.join(format!(
        "{}/{}/{}",
        sync_state.fs_safe_name,
        sync_state.sync_item,
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
    ));

    log::info!("backing up to {:?}", root_dir);

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

    log::info!("backup complete");

    Ok(())
}
