use crate::ctr_fs::{self, CtrArchive};
use crate::db::StateDb;
use crate::services::CtrSysServices;
use crate::settings::SETTINGS;
use anyhow::{Context, Result};
use cloudpoint_lib::ctr::CtrArchiveKind;
use cloudpoint_lib::sync::SyncState;
use ctru::services::fs::MediaType;
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

pub fn append_discovered(services: &CtrSysServices, state_db: &mut StateDb) -> Result<()> {
    log::debug!("discovering savedata and extdata");

    let installed_titles = services.am.title_list(MediaType::Sd)?;

    for title in installed_titles {
        let title_id = title.id();

        log::debug!("processing {title_id:016X}");

        for archive_kind in [CtrArchiveKind::Savedata, CtrArchiveKind::Extdata] {
            if state_db.contains_state(title_id, archive_kind) {
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

                state_db.insert_state(title_id, archive_kind, state);
            }
        }
    }

    Ok(())
}
