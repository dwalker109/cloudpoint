use crate::{
    ctr_fs::{self, CtrArchive},
    ctr_title::{infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title},
    services::CtrSysServices,
};
use anyhow::Result;
use cloudpoint_lib::sync::{SyncItem, SyncState};
use ctru::services::fs::MediaType;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// These titles don't support sync to another system so are skipped
/// during discovery. They *can* be synced if added manually.
pub const SKIPPED_TITLE_IDS: [u64; 1] = [
    // Super Mario Maker
    0x00040000001A0500,
];

pub struct StateDb(PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(path: impl AsRef<Path>, _services: &CtrSysServices) -> Result<Self> {
        let mut states: Vec<SyncState> = Vec::new();

        for f in fs::read_dir(&path)? {
            let f = f?;
            if let Ok(s) = postcard::from_bytes(&fs::read(f.path())?) {
                states.push(s);
            }
        }

        Ok(Self(
            path.as_ref().to_path_buf(),
            states.into_iter().map(|s| (s.sync_item, s)).collect(),
        ))
    }

    pub fn append_discovered(&mut self, services: &CtrSysServices) -> Result<()> {
        log::debug!("discovering savedata and extdata");

        let installed_titles = services.am.title_list(MediaType::Sd)?;
        let installed_apps = installed_titles
            .iter()
            .filter(|t| (t.id() >> 32) as u32 == 0x00040000);

        for title in installed_apps {
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

                    println!("Discovered {} via {}", state.sync_item, state.title_short);

                    self.1.insert(archive_id, state);
                }

                Ok(())
            };

            let sync_item = SyncItem::Savedata(title_id);
            process(sync_item)?;

            if let Some(archive_id) = lookup_extdata_sync_item_for_title(title_id) {
                process(archive_id)?;
            } else if let Some(archive_id) = infer_extdata_sync_item_for_title(title_id) {
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
