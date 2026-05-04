use crate::ctr::{CtrSmdh, SmdhLanguage};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
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
    pub title_short: String,
    pub title_publisher: String,
    pub product_code: String,
    pub fs_safe_name: String,
    pub last_fp: Option<u128>,
    #[serde(skip)]
    pub local_fp: Option<u128>,
    #[serde(skip)]
    pub remote_fp: Option<u128>,
}

impl SyncState {
    pub fn new(sync_item: SyncItem, product_code: &str, smdh: &CtrSmdh) -> Self {
        let title_short = smdh.title_short(SmdhLanguage::English);
        let title_publisher = smdh.title_publisher(SmdhLanguage::English);
        let product_code = product_code.trim_end_matches('\0').to_string();

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
            title_short,
            title_publisher,
            product_code,
            fs_safe_name,
            last_fp: None,
            local_fp: None,
            remote_fp: None,
        }
    }

    pub fn save(&self, root_path: impl AsRef<Path>) -> Result<()> {
        log::info!("Writing db for {} ({})", self.sync_item, self.title_short);

        fs::write(
            root_path.as_ref().join(PathBuf::from(self.sync_item)),
            postcard::to_allocvec(&self)?,
        )?;

        Ok(())
    }

    pub fn get_action(&self) -> SyncAction {
        match (self.local_fp, self.remote_fp) {
            (None, None) => unreachable!(),
            (None, Some(_)) => SyncAction::Download,
            (Some(_), None) => SyncAction::Upload,
            (Some(l), Some(r)) if l == r => SyncAction::NoChange,
            (Some(_), Some(_)) => {
                let changed_local = self.local_fp != self.last_fp;
                let changed_remote = self.remote_fp != self.last_fp;

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
        let mut s = SyncState { ..fixture() };
        s.local_fp = Some(1);

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_only_no_local() {
        let mut s = SyncState { ..fixture() };
        s.remote_fp = Some(1);

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn local_change_only() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(2),
            remote_fp: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_change_only() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(1),
            remote_fp: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn both_change() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(2),
            remote_fp: Some(3),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Conflict));
    }

    #[test]
    fn no_local_with_remote_always_download() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: None,
            remote_fp: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn no_remote_with_local_always_upload() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(1),
            remote_fp: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    #[should_panic]
    fn no_remote_no_local_cannot_happen() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: None,
            remote_fp: None,
            ..fixture()
        };

        s.get_action();
    }

    #[test]
    fn matching_local_and_remote_always_no_change() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(2),
            remote_fp: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::NoChange));
    }

    fn fixture() -> SyncState {
        SyncState {
            sync_item: SyncItem::Savedata(0x00040000_1234ABCD),
            title_short: "Foo Bar: Yeah!".into(),
            title_publisher: "Cloudpoint, Inc.".into(),
            product_code: "XTR-X-ABCD".into(),
            fs_safe_name: "Foo Bar  Yeah ".into(),
            last_fp: None,
            local_fp: None,
            remote_fp: None,
        }
    }
}
