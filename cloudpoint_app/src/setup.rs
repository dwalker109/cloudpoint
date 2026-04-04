use crate::services::CtrSysServices;
use crate::settings::SETTINGS;
use anyhow::{Context, Result};
use cloudpoint_lib::sync::{CtrArchiveKind, SyncState};
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
    let mut sync_states = load_db_all(&installed_titles)?;
    append_autoadd(&installed_titles, &mut sync_states)?;

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

fn append_autoadd(
    installed_titles: &[Title],
    sync_states: &mut HashMap<(u64, CtrArchiveKind), SyncState>,
) -> Result<()> {
    let titles = installed_titles
        .iter()
        .map(|t| (t.product_code().trim_end_matches('\0').to_string(), t.id()))
        .collect::<HashMap<_, _>>();

    for (product_code, archive_kind) in
        fs::read_to_string(format!("sdmc:/3ds/Cloudpoint/autoadd.txt"))?
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
