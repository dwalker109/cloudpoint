use crate::{
    config::AppPath,
    ctr_fs::{self, CtrArchive},
    ctr_title::{self, infer_extdata_sync_item_for_title, lookup_extdata_sync_item_for_title},
    title::{TitleDetails, TitleSyncStatus},
};
use anyhow::Result;
use cloudpoint_lib::sync::{SyncItem, SyncState};
use ctru::services::{am::Am, fs::MediaType};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

/// These titles don't support sync to another system so are added but not enabled
/// during discovery. They *can* be synced if later enabled manually.
pub const UNSUPPORTED_TITLE_IDS: [u64; 1] = [
    // Super Mario Maker
    0x00040000001A0500,
];

pub struct StateDb(PathBuf, HashMap<SyncItem, SyncState>);

impl StateDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
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

    pub fn append_discovered(&mut self) -> Result<()> {
        log::info!("discovering savedata and extdata");

        let am = Am::new()?;
        let installed_titles = am.title_list(MediaType::Sd)?;
        let installed_apps = installed_titles
            .iter()
            .filter(|t| (t.id() >> 32) as u32 == 0x00040000);

        for title in installed_apps {
            let title_id = title.id();

            log::info!("processing {title_id:016X}");

            let mut process = |sync_item| -> Result<()> {
                if let Some(existing_state) = self.1.get_mut(&sync_item) {
                    if existing_state.add_via_title_id(title_id) {
                        log::info!("updating {sync_item} reached via {title_id:016X}");
                        existing_state.save(AppPath::Db)?;
                    } else {
                        log::info!(
                            "skipping {sync_item} discovered via {title_id:016X}, already tracked"
                        );
                    }

                    return Ok(());
                }

                if ctr_fs::CtrArchive::open(sync_item).is_ok() {
                    log::info!("adding {sync_item} discovered via {title_id:016X}");

                    let mut state = SyncState::new(
                        sync_item,
                        title_id,
                        &title.product_code(),
                        &CtrArchive::smdh(sync_item)?,
                        !UNSUPPORTED_TITLE_IDS.contains(&title_id),
                    );
                    state.save(AppPath::Db)?;

                    self.1.insert(sync_item, state);
                }

                Ok(())
            };

            let sync_item = SyncItem::Savedata(title_id);
            process(sync_item)?;

            if let Some(sync_item) = lookup_extdata_sync_item_for_title(title_id) {
                process(sync_item)?;
            } else if let Some(sync_item) = infer_extdata_sync_item_for_title(title_id) {
                process(sync_item)?;
            }
        }

        Ok(())
    }

    pub fn total_states(&self) -> usize {
        self.1.len()
    }

    pub fn state(&self, sync_item: &SyncItem) -> Option<&SyncState> {
        self.1.get(sync_item)
    }

    pub fn states(&self) -> impl Iterator<Item = &SyncState> {
        self.1.values()
    }

    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut SyncState> {
        self.1.values_mut()
    }

    pub fn save_all(&mut self) -> Result<()> {
        for state in self.1.values_mut() {
            state.save(&self.0)?
        }

        Ok(())
    }
}

pub struct TitleDb(Vec<TitleDetails>);

impl TitleDb {
    pub fn build(state_db: Arc<RwLock<StateDb>>) -> Result<Self> {
        log::info!("building runtime title db");

        let mut titles = Vec::new();

        let am = Am::new()?;
        let installed_titles = am.title_list(MediaType::Sd)?;
        let installed_apps = installed_titles
            .iter()
            .filter(|t| (t.id() >> 32) as u32 == 0x00040000);

        for title in installed_apps {
            let title_id = title.id();
            let product_code = title.product_code();
            let smdh = ctr_title::smdh(title_id)?;

            log::info!("processing {title_id:016X}");

            let title = TitleDetails::new(
                title_id,
                &product_code,
                smdh,
                &state_db.read().expect("should get read lock for state db"),
            );

            if title.savedata_sync_status != TitleSyncStatus::Unavailable
                || title.extdata_sync_status != TitleSyncStatus::Unavailable
            {
                log::debug!("added {title_id:016X}");
                titles.push(title);
            } else {
                log::debug!("ignored {title_id:016X}, has no save or extdata");
            }
        }

        titles.sort_by_key(|t| {
            t.smdh
                .title_short(cloudpoint_lib::ctr::SmdhLanguage::English)
        });

        Ok(Self(titles))
    }

    pub fn total_titles(&self) -> usize {
        self.0.len()
    }

    pub fn titles(&self) -> impl Iterator<Item = &TitleDetails> {
        self.0.iter()
    }
}
