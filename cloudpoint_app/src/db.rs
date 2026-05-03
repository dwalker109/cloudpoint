use crate::{
    ctr_fs::{self, CtrArchive},
    ctr_title::extdata_archive_id_for_title,
    services::CtrSysServices,
};
use anyhow::Result;
use cloudpoint_lib::{ctr::CtrArchiveId, sync::SyncState};
use ctru::services::fs::MediaType;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// These titles don't support sync to another system so are skipped
/// during discovery. They *can* be synced if added manually.
pub const SKIPPED_TITLE_IDS: [u64; 3] = [
    // Mii Plaza
    0x0004000E00022800,
    // Super Mario Maker
    0x00040000001A0500,
    0x0004000E001A0500,
];

pub struct StateDb(PathBuf, HashMap<CtrArchiveId, SyncState>);

impl StateDb {
    pub fn open(path: impl AsRef<Path>, _services: &CtrSysServices) -> Result<Self> {
        // let installed_titles = services.am.title_list(MediaType::Sd)?;
        // let title_ids = installed_titles.iter().map(|t| t.id()).collect::<Vec<_>>();

        let mut states: Vec<SyncState> = Vec::new();

        for f in fs::read_dir(&path)? {
            let f = f?;
            if let Ok(s) = postcard::from_bytes(&fs::read(f.path())?) {
                states.push(s);
            }
        }

        Ok(Self(
            path.as_ref().to_path_buf(),
            states
                .into_iter()
                // .filter(|s| title_ids.contains(&s.title_id))
                .map(|s| (s.archive_id, s))
                .collect(),
        ))
    }

    pub fn append_discovered(&mut self, services: &CtrSysServices) -> Result<()> {
        log::debug!("discovering savedata and extdata");

        let installed_titles = services.am.title_list(MediaType::Sd)?;

        for title in installed_titles {
            let title_id = title.id();

            log::debug!("processing {title_id:016X}");

            if SKIPPED_TITLE_IDS.contains(&title_id) {
                log::debug!("skipping title, unsupported");
                println!("Skipping {:016X}, not supported", title_id);
                continue;
            }

            let mut process = |archive_id| -> Result<()> {
                if self.1.contains_key(&archive_id) {
                    log::debug!("skipping {archive_id} discovered via {title_id}, already tracked");

                    return Ok(());
                }

                if ctr_fs::CtrArchive::open(archive_id).is_ok() {
                    log::debug!("adding {archive_id} discovered via {title_id}");

                    let state = SyncState::new(
                        archive_id,
                        &title.product_code(),
                        &CtrArchive::smdh(archive_id)?,
                    );

                    println!("Discovered {} via {}", state.archive_id, state.title_short);

                    self.1.insert(archive_id, state);
                }

                Ok(())
            };

            let archive_id = CtrArchiveId::Savedata(title_id);
            process(archive_id)?;

            if let Some(archive_id) = extdata_archive_id_for_title(title_id) {
                process(archive_id)?;
            }
        }

        Ok(())
    }

    pub fn total_states(&self) -> usize {
        self.1.len()
    }

    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut SyncState> {
        self.1.values_mut()
    }

    pub fn save_all(&self) -> Result<()> {
        for state in self.1.values() {
            state.save(&self.0)?
        }

        Ok(())
    }
}
