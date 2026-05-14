use crate::{
    app::{ModalMsg, UiMsg},
    config::{AppPath, USER_KEY, USER_SETTINGS},
    ctr_fs::CtrArchive,
    ctr_ndmu::KeepAwake,
    ctr_title::meta,
    tree::{self, CtrArchiveLeaf},
};
use anyhow::{Result, bail};
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
use ctru::services::ac::Ac;
use std::{
    fs::{self, File},
    io::{self, BufWriter},
    path::PathBuf,
    rc::Rc,
    sync::{mpsc::Sender, oneshot},
};

pub enum ConflictWinner {
    Local,
    Remote,
    Undecided,
}

pub fn run<'a>(
    states: impl Iterator<Item = &'a mut SyncState>,
    ui_tx: Sender<UiMsg>,
    alert_tx: Sender<ModalMsg>,
    client: &Rc<CurlHttpClient>,
) -> Result<()> {
    let _keep_awake = KeepAwake::new();

    for sync_state in states {
        run_one(sync_state, &ui_tx, &alert_tx, &client)?;
    }

    Ok(())
}

pub fn run_one(
    sync_state: &mut SyncState,
    ui_tx: &Sender<UiMsg>,
    alert_tx: &Sender<ModalMsg>,
    client: &Rc<CurlHttpClient>,
) -> Result<()> {
    if Ac::new()?.wait_internet_connection().is_err() {
        bail!("You are not connected to the internet");
    }

    let Ok(smdh) = CtrArchive::smdh(sync_state.sync_item) else {
        log::info!(
            "{} archive does not exist, cannot sync",
            sync_state.sync_item,
        );
        bail!("{} not found; was the title deleted?", sync_state.sync_item);
    };

    let title_label = format!(
        "{} ({})",
        smdh.title_short(SmdhLanguage::English),
        smdh.title_publisher(SmdhLanguage::English)
    );

    log::info!("Starting sync of {title_label}",);

    ui_tx
        .send(UiMsg::SyncProgress {
            title_lbl: title_label.clone(),
            message: "Checking".into(),
        })
        .ok();

    let list = cloudpoint_lib::version::VersionDirList::try_get(
        &client,
        &USER_SETTINGS.base_url,
        &USER_KEY,
        sync_state.sync_item,
    )?;

    let remote_ver = list.latest();
    let remote_fingerprint = remote_ver.and_then(|e| e.fingerprint().ok());

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
        SyncAction::Skip => {
            log::info!("ignoring {}, is not enabled", sync_state.sync_item,);
        }
        SyncAction::NoChange | SyncAction::NoChangeOnInit => {
            log::info!("local and remote data match for {}", sync_state.sync_item,);

            if sync_state.synced_fingerprint.is_none() {
                sync_state.synced_fingerprint = local_fingerprint;
            }
        }
        SyncAction::Conflict | SyncAction::ConflictOnInit => {
            log::info!("changed on server and locally for {}", sync_state.sync_item,);

            let is_first_sync = sync_state.synced_fingerprint.is_none();

            let (reply_tx, reply_rx) = oneshot::channel::<ConflictWinner>();

            alert_tx
                .send(ModalMsg::ResolveConflict {
                    title_label: title_label.clone(),
                    title_remote_time: remote_ver.map(|v| v.mtime().clone()),
                    is_first_sync,
                    reply_tx,
                })
                .ok();

            match reply_rx.recv()? {
                ConflictWinner::Local => {
                    ul(
                        sync_state,
                        Rc::clone(&client),
                        &ui_tx,
                        &title_label,
                        &local_ver,
                        &local_tree,
                        local_fingerprint,
                    )?;
                }
                ConflictWinner::Remote => {
                    dl(
                        sync_state,
                        Rc::clone(&client),
                        &ui_tx,
                        &title_label,
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
                &ui_tx,
                &title_label,
                &local_ver,
                &local_tree,
                local_fingerprint,
            )?;
        }
        SyncAction::Download => {
            dl(
                sync_state,
                Rc::clone(&client),
                &ui_tx,
                &title_label,
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
    ui_tx: &Sender<UiMsg>,
    title_label: &str,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
    local_tree: &Tree<CtrArchiveLeaf>,
    local_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("Uploading {}", s.sync_item);

    ui_tx
        .send(UiMsg::SyncProgress {
            title_lbl: title_label.into(),
            message: "Uploading".into(),
        })
        .ok();

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
        s.sync_item,
        &local_ver,
    )?;

    s.synced_fingerprint = local_fingerprint;

    Ok(())
}

fn dl(
    s: &mut SyncState,
    client: Rc<CurlHttpClient>,
    ui_tx: &Sender<UiMsg>,
    title_label: &str,
    archive: Rc<CtrArchive>,
    local_meta: &CtrMeta,
    local_ver: &Version<CtrArchiveLeaf, CtrMeta>,
    local_tree: Tree<CtrArchiveLeaf>,
    remote_fingerprint: Option<u128>,
) -> Result<()> {
    log::info!("Downloading {}", s.sync_item);

    let remote_ver = VersionDirEntry::get_version::<CtrArchiveLeaf, CtrMeta>(
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
        ui_tx
            .send(UiMsg::SyncProgress {
                title_lbl: title_label.into(),
                message: "Backing up existing data".into(),
            })
            .ok();

        backup(&local_tree, &s)?;
    }

    ui_tx
        .send(UiMsg::SyncProgress {
            title_lbl: title_label.into(),
            message: "Downloading".into(),
        })
        .ok();

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
            "error occurred while downloading the version: {:?}",
            u.progress()
        );

        bail!("Something went wrong downloading the remote version");
    }

    archive.finalise()?;

    s.synced_fingerprint = remote_fingerprint;

    Ok(())
}

fn backup(local_tree: &Tree<CtrArchiveLeaf>, sync_state: &SyncState) -> Result<()> {
    let root_dir = AppPath::Backup.join(format!(
        "{}/{}/{}",
        sync_state.fs_safe_name,
        sync_state.sync_item,
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
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
