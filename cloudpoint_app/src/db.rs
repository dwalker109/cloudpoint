use crate::{
    ctr_fs::{self, CtrArchive},
    services::CtrSysServices,
};
use anyhow::Result;
use cloudpoint_lib::{ctr::CtrArchiveKind, sync::SyncState};
use ctru::services::fs::MediaType;
use std::{
    collections::{HashMap, HashSet},
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

pub struct StateDb(PathBuf, HashMap<(u64, CtrArchiveKind), SyncState>);

impl StateDb {
    pub fn open(path: impl AsRef<Path>, services: &CtrSysServices) -> Result<Self> {
        let installed_titles = services.am.title_list(MediaType::Sd)?;
        let ids = installed_titles.iter().map(|t| t.id()).collect::<Vec<_>>();

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
                .filter(|s| ids.contains(&s.title_id))
                .map(|s| ((s.title_id, s.archive_kind), s))
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
                println!("Skipping {:016X}, is tied to this console", title_id);
                continue;
            }

            for archive_kind in [CtrArchiveKind::Savedata, CtrArchiveKind::Extdata] {
                if self.contains_state(title_id, archive_kind) {
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

                    self.insert_state(title_id, archive_kind, state);
                }
            }
        }

        Ok(())
    }

    pub fn total_states(&self) -> usize {
        self.1.len()
    }

    pub fn total_titles(&self) -> usize {
        self.1
            .values()
            .map(|s| s.title_id)
            .collect::<HashSet<_>>()
            .len()
    }

    pub fn contains_state(&self, title_id: u64, archive_kind: CtrArchiveKind) -> bool {
        self.1.contains_key(&(title_id, archive_kind))
    }

    pub fn insert_state(&mut self, title_id: u64, archive_kind: CtrArchiveKind, state: SyncState) {
        self.1.insert((title_id, archive_kind), state);
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
