use crate::ctr::{CtrSmdh, SmdhLanguage};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;

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
    pub auto_enabled: bool,
    pub title_short: String,
    pub title_publisher: String,
    pub fs_safe_name: String,
    pub synced_fingerprint: Option<u128>,
    pub via_title_ids: HashSet<u64>,
    pub via_user_key: Uuid,
}

impl SyncState {
    pub fn new(sync_item: SyncItem, via_title_id: u64, smdh: &CtrSmdh, auto_enabled: bool) -> Self {
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
            auto_enabled,
            title_short,
            title_publisher,
            fs_safe_name,
            synced_fingerprint: None,
            via_title_ids: HashSet::from([via_title_id]),
            via_user_key: Uuid::nil(),
        }
    }

    pub fn safe_adopt(&mut self, user_key: Uuid) {
        if self.via_user_key != user_key {
            self.synced_fingerprint = None;
            self.via_user_key = user_key;
        }
    }

    pub fn get_action(
        &self,
        local_fingerprint: Option<u128>,
        remote_fingerprint: Option<u128>,
    ) -> SyncAction {
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
    fn unmatched_user_key_resets_sync_state() {
        let via_user_key = Uuid::new_v4();
        let mut s = SyncState {
            synced_fingerprint: Some(u128::MAX),
            via_user_key,
            ..fixture()
        };

        s.safe_adopt(Uuid::new_v4());

        assert!(s.synced_fingerprint.is_none());
        assert!(s.via_user_key != via_user_key);
    }

    fn fixture() -> SyncState {
        SyncState {
            sync_item: SyncItem::Savedata(0x00040000_1234ABCD),
            auto_enabled: true,
            via_title_ids: HashSet::new(),
            title_short: "Foo Bar: Yeah!".into(),
            title_publisher: "Cloudpoint, Inc.".into(),
            fs_safe_name: "Foo Bar  Yeah ".into(),
            synced_fingerprint: None,
            via_user_key: Uuid::max(),
        }
    }
}
