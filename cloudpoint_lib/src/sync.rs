use crate::ctr::{CtrSmdh, SmdhLanguage};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum SyncItem {
    Savedata(u64),
    Extdata(u64),
}

impl std::fmt::Display for SyncItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncItem::Savedata(title_id) => write!(f, "{title_id:016X} savedata"),
            SyncItem::Extdata(extdata_id) => write!(f, "{extdata_id:016X} extdata"),
        }
    }
}

impl From<SyncItem> for PathBuf {
    fn from(value: SyncItem) -> Self {
        match value {
            SyncItem::Savedata(title_id) => PathBuf::from(format!("{title_id:016X}.savedata")),
            SyncItem::Extdata(extdata_id) => PathBuf::from(format!("{extdata_id:016X}.extdata")),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncState {
    pub sync_item: SyncItem,
    pub enabled: bool,
    pub title_short: String,
    pub title_publisher: String,
    pub fs_safe_name: String,
    pub synced_fingerprint: Option<u128>,
    pub via_title_ids: HashSet<u64>,
}

impl SyncState {
    pub fn new(sync_item: SyncItem, via_title_id: u64, smdh: &CtrSmdh, enabled: bool) -> Self {
        let title_short = smdh.title_short(SmdhLanguage::English);
        let title_publisher = smdh.title_publisher(SmdhLanguage::English);

        let illegal = r#".,!\\/:?*"<>|"#;
        let fs_safe_name = title_short
            .chars()
            .map(|c| illegal.contains(c).then_some(' ').or(Some(c)))
            .flatten()
            .collect::<String>()
            .trim_end()
            .to_owned();

        Self {
            sync_item,
            enabled,
            title_short,
            title_publisher,
            fs_safe_name,
            synced_fingerprint: None,
            via_title_ids: HashSet::from([via_title_id]),
        }
    }

    pub fn save(&mut self, root_path: impl AsRef<Path>) -> Result<()> {
        log::info!("Writing db for {} ({})", self.sync_item, self.title_short);

        fs::write(
            root_path.as_ref().join(PathBuf::from(self.sync_item)),
            postcard::to_allocvec(&self)?,
        )?;

        Ok(())
    }

    pub fn add_via_title_id(&mut self, via: u64) -> bool {
        self.via_title_ids.insert(via)
    }

    pub fn get_action(
        &self,
        local_fingerprint: Option<u128>,
        remote_fingerprint: Option<u128>,
    ) -> SyncAction {
        if !self.enabled {
            return SyncAction::Skip;
        }

        match (local_fingerprint, remote_fingerprint) {
            (None, None) => unreachable!(),
            (None, Some(_)) => SyncAction::Download,
            (Some(_), None) => SyncAction::Upload,
            (Some(l), Some(r)) if l == r => SyncAction::NoChange,
            (Some(_), Some(_)) => {
                let changed_local = local_fingerprint != self.synced_fingerprint;
                let changed_remote = remote_fingerprint != self.synced_fingerprint;

                match (changed_local, changed_remote) {
                    (false, true) => SyncAction::Download,
                    (true, false) => SyncAction::Upload,
                    (true, true) => SyncAction::Conflict,
                    (false, false) => unreachable!(),
                }
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SyncAction {
    Skip,
    NoChange,
    NoChangeOnInit,
    Conflict,
    ConflictOnInit,
    Upload,
    Download,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_only_no_remote() {
        let s = SyncState { ..fixture() };

        let res = s.get_action(Some(1), None);

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_only_no_local() {
        let s = SyncState { ..fixture() };

        let res = s.get_action(None, Some(1));

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn local_change_only() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(Some(2), Some(1));

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_change_only() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(Some(1), Some(2));

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn both_change() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(Some(2), Some(3));

        assert!(matches!(res, SyncAction::Conflict));
    }

    #[test]
    fn no_local_with_remote_always_download() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(None, Some(1));

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn no_remote_with_local_always_upload() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(Some(1), None);

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    #[should_panic]
    fn no_remote_no_local_cannot_happen() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        s.get_action(None, None);
    }

    #[test]
    fn matching_local_and_remote_always_no_change() {
        let s = SyncState {
            synced_fingerprint: Some(1),
            ..fixture()
        };

        let res = s.get_action(Some(2), Some(2));

        assert!(matches!(res, SyncAction::NoChange));
    }

    #[test]
    fn skip_when_not_enabled() {
        let s = SyncState {
            enabled: false,
            ..fixture()
        };

        let res = s.get_action(Some(1), Some(2));

        assert!(matches!(res, SyncAction::Skip));
    }

    fn fixture() -> SyncState {
        SyncState {
            sync_item: SyncItem::Savedata(0x00040000_1234ABCD),
            enabled: true,
            via_title_ids: HashSet::new(),
            title_short: "Foo Bar: Yeah!".into(),
            title_publisher: "Cloudpoint, Inc.".into(),
            fs_safe_name: "Foo Bar  Yeah ".into(),
            synced_fingerprint: None,
        }
    }
}
