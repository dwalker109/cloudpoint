use crate::services::CtrSysServices;
use anyhow::Result;
use cloudpoint_lib::{ctr::CtrArchiveKind, sync::SyncState};
use ctru::services::fs::MediaType;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

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
