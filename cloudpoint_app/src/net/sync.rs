use crate::{
    app::{ConflictWinner, OpenModalMsg, SyncProgress, UiMsg},
    config::{AppPath, USER_KEY, USER_SETTINGS},
    ctr_fs::CtrLeaf,
    db::{InstallHistoryDb, InstallStatus, TitleDb},
    ndmu::KeepAwake,
    net::{HTTP_CLIENT, sync::local_version::LocalVersionInfo},
};
use anyhow::{Result, bail};
use chrono::Utc;
use chunktree::{
    store::MemStore,
    tree::{Leaf, Tree},
    version::{Diff, updater::BlockingUpdater},
};
use cloudpoint_lib::{
    ctr::CtrMeta,
    http::CurlHttpClient,
    remote_version::{RemoteVersionInfo, get_version, put_version},
    store::HttpStore,
    sync::{SyncAction, SyncState},
};
use ctru::services::ac::Ac;
use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
};

mod local_version;

pub fn run<'a>(
    states: impl Iterator<Item = &'a mut SyncState>,
    title_db: &TitleDb,
    shutdown_rx: &Receiver<()>,
    ui_tx: Sender<UiMsg>,
    modal_tx: Sender<OpenModalMsg>,
    install_history_db: &mut InstallHistoryDb,
) -> Result<usize> {
    log::info!("starting sync");

    let _keep_awake = KeepAwake::new();
    let mut sync_progress = SyncProgress::new(ui_tx);

    let states = states.collect::<Vec<_>>();
    let qty_total = states.len();
    let mut qty_completed = 0;

    for (i, sync_state) in states.into_iter().enumerate() {
        if let Err(mpsc::TryRecvError::Disconnected) = shutdown_rx.try_recv() {
            log::info!("aborting mid sync due to app shutdown");
            return Ok(qty_completed);
        }

        match run_one(
            sync_state,
            &title_db.sync_state_label(&sync_state),
            &mut sync_progress,
            &modal_tx,
            install_history_db,
        ) {
            Ok(_) => sync_progress.progress((i + 1) * 100 / qty_total),
            Err(e) => {
                log::error!("failed mid sync: {e}");
                return Err(e);
            }
        };

        qty_completed += 1;
    }

    log::info!("completed sync of {qty_completed} sync states");

    Ok(qty_completed)
}

fn run_one(
    sync_state: &mut SyncState,
    title_label: &str,
    sync_progress: &mut SyncProgress,
    modal_tx: &Sender<OpenModalMsg>,
    install_history_db: &mut InstallHistoryDb,
) -> Result<()> {
    log::info!("Starting sync of {}", sync_state.sync_item);

    if Ac::new()?.wait_internet_connection().is_err() {
        log::warn!("internet down");
        bail!("No internet connection is available - please ensure you are online and try again");
    }

    sync_progress.label(title_label).message("Checking").send();

    for title_id in &sync_state.via_title_ids {
        match install_history_db.check(*title_id, sync_state.sync_item) {
            InstallStatus::Updated => {
                log::info!("via_title_id {title_id:016X}: updated tmd mtime, reset sync meta");
                sync_state.synced_at = None;
                sync_state.synced_fingerprint = None;
                install_history_db.touch(*title_id, sync_state.sync_item);
            }
            InstallStatus::Unchanged => {
                log::debug!("via_title_id {title_id:016X}: unchanged tmd mtime, leave sync meta");
            }
        }
    }

    if sync_state.via_user_key != *USER_KEY {
        log::info!("user.key has changed, adopting (new sync dialog is normal)");

        sync_state.synced_at = None;
        sync_state.synced_fingerprint = None;
        sync_state.via_user_key = *USER_KEY;
    }

    let remote_ver_info = RemoteVersionInfo::latest(
        &HTTP_CLIENT,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        sync_state.sync_item,
    )?;
    let remote_fingerprint = remote_ver_info.as_ref().and_then(|m| m.fingerprint().ok());

    let local_ver_info = LocalVersionInfo::try_new(sync_state.sync_item)?;
    let local_fingerprint = local_ver_info.fingerprint();

    log::debug!("Local: {:032x}", local_fingerprint.unwrap_or_default());
    log::debug!("Remote: {:032x}", remote_fingerprint.unwrap_or_default());

    match sync_state.get_action(local_fingerprint, remote_fingerprint) {
        SyncAction::NoChange => {
            log::info!("local and remote data match for {}", sync_state.sync_item,);

            if sync_state.synced_fingerprint.is_none() {
                sync_state.synced_fingerprint = local_fingerprint;
            }

            sync_state.synced_at = Some(Utc::now().naive_utc());
        }
        SyncAction::Conflict => {
            log::info!("changed on server and locally for {}", sync_state.sync_item,);

            let (reply_tx, reply_rx) = oneshot::channel::<ConflictWinner>();

            modal_tx
                .send(OpenModalMsg::ResolveConflict {
                    label: title_label.to_string(),
                    local_synced_at: sync_state.synced_at,
                    remote_synced_at: remote_ver_info.as_ref().map(|v| v.created_at),
                    reply_tx,
                })
                .ok();

            match reply_rx.recv()? {
                ConflictWinner::Local => {
                    ul(
                        sync_state,
                        &HTTP_CLIENT,
                        sync_progress,
                        local_ver_info,
                        local_fingerprint,
                    )?;
                }
                ConflictWinner::Remote => {
                    dl(
                        sync_state,
                        &HTTP_CLIENT,
                        sync_progress,
                        local_ver_info,
                        remote_fingerprint,
                    )?;
                }
                ConflictWinner::Undecided => {}
            };
        }
        SyncAction::Upload => {
            ul(
                sync_state,
                &HTTP_CLIENT,
                sync_progress,
                local_ver_info,
                local_fingerprint,
            )?;
        }
        SyncAction::Download => {
            dl(
                sync_state,
                &HTTP_CLIENT,
                sync_progress,
                local_ver_info,
                remote_fingerprint,
            )?;
        }
    }

    log::info!("sync completed for {}", sync_state.sync_item);

    Ok(())
}

fn ul(
    s: &mut SyncState,
    client: &CurlHttpClient,
    sync_progress: &mut SyncProgress,
    local_ver_info: LocalVersionInfo,
    local_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("uploading {}", s.sync_item);

    sync_progress.message("Uploading").send();

    let (_local_meta, local_tree, local_ver) = local_ver_info.into_parts()?;

    let mut store = HttpStore::new(
        client.clone(),
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
    s.synced_at = Some(Utc::now().naive_utc());

    Ok(())
}

fn dl(
    s: &mut SyncState,
    client: &CurlHttpClient,
    sync_progress: &mut SyncProgress,
    local_ver_info: LocalVersionInfo,
    remote_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("downloading {}", s.sync_item);

    let (local_meta, local_tree, local_ver) = local_ver_info.into_parts()?;

    let remote_ver = get_version::<CtrLeaf, CtrMeta>(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        s.sync_item,
        remote_fingerprint.expect("remote_fingerprint should be Some<u128> to init a download"),
    )?;

    if local_meta.required_version() != remote_ver.meta().required_version() {
        log::warn!(
            "title versions do not match, but sync will attempt to continue: local={:?} remote={:?}",
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
        client.clone(),
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

    let (_, local_tree, _, _) = u.try_unwrap()?;
    local_tree.context().finalise()?;

    s.synced_fingerprint = remote_fingerprint;
    s.synced_at = Some(Utc::now().naive_utc());

    Ok(())
}

fn backup(tree: &Tree<CtrLeaf>, sync_state: &SyncState) -> Result<()> {
    let root_dir = AppPath::Backup.join(format!(
        "{}/{}",
        sync_state.sync_item,
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
    ));

    log::info!("backing up to {:?}", root_dir);

    for leaf in tree.leaves() {
        let dst_path: PathBuf = root_dir.join(
            leaf.path(tree.context())
                .components()
                .filter(|c| matches!(c, std::path::Component::Normal(_)))
                .collect::<PathBuf>(),
        );

        fs::create_dir_all(dst_path.parent().expect("file has parent directory"))?;

        let mut w = BufWriter::with_capacity(256 * 1024, File::create(dst_path)?);
        io::copy(&mut leaf.data(tree.context())?, &mut w)?;
        w.flush()?;
    }

    log::info!("backup complete");

    Ok(())
}
