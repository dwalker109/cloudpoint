use crate::ctr_fs::{self, CtrArchive};
use crate::services::CtrSysServices;
use anyhow::{Context, Result};
use cloudpoint_lib::ctr::CtrArchiveKind;
use cloudpoint_lib::settings::SETTINGS;
use cloudpoint_lib::sync::SyncState;
use ctru::services::am::{Am, Title};
use ctru::services::fs::MediaType;
use std::collections::HashMap;
use std::fs;

pub fn logging() -> Result<()> {
    flexi_logger::Logger::try_with_str(&SETTINGS.log)?
        .log_to_file(flexi_logger::FileSpec::default().directory("sdmc:/3ds/Cloudpoint/logs"))
        .start()?;

    Ok(())
}

pub fn sdmc() -> Result<()> {
    let paths = [
        "sdmc:/3ds/Cloudpoint",
        "sdmc:/3ds/Cloudpoint/db",
        "sdmc:/3ds/Cloudpoint/logs",
    ];
    for p in paths {
        fs::create_dir_all(p).with_context(|| format!("fatal: failed to create directory {p}"))?;
    }

    log::debug!("Created paths");

    Ok(())
}

pub fn sync_states(services: &CtrSysServices) -> Result<HashMap<(u64, CtrArchiveKind), SyncState>> {
    let installed_titles = get_installed_titles(&services.am)?;
    let sync_states = load_db_all(&installed_titles)?;

    log::info!("Loaded {} sync states", sync_states.len());

    Ok(sync_states)
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

pub fn append_discovered(
    services: &CtrSysServices,
    sync_states: &mut HashMap<(u64, CtrArchiveKind), SyncState>,
) -> Result<()> {
    log::debug!("discovering savedata and extdata");

    let installed_titles = get_installed_titles(&services.am)?;

    for title in installed_titles {
        let title_id = title.id();

        log::debug!("processing {title_id:016X}");

        for archive_kind in [CtrArchiveKind::Savedata, CtrArchiveKind::Extdata] {
            if sync_states.contains_key(&(title_id, archive_kind)) {
                log::debug!("skipping {archive_kind}, already tracked");
                continue;
            }

            if ctr_fs::CtrArchive::open(title_id, archive_kind).is_ok() {
                log::debug!("adding {archive_kind}");

                let state = SyncState::new(
                    title_id,
                    &title.product_code(),
                    &CtrArchive::smdh(title_id, archive_kind)?,
                    archive_kind,
                );

                println!("Discovered {} {}", state.title_short, archive_kind);

                sync_states.insert((title_id, archive_kind), state);
            }
        }
    }

    Ok(())
}
