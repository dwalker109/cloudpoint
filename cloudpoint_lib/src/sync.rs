use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub title_id: u64,
    pub product_code: String,
    pub archive_kind: CtrArchiveKind,
    pub last_fp: Option<u64>,
    #[serde(skip)]
    pub local_fp: Option<u64>,
    #[serde(skip)]
    pub remote_fp: Option<u64>,
}

impl SyncState {
    pub fn get_action(&self) -> SyncAction {
        match (self.local_fp, self.remote_fp) {
            (None, Some(_)) => SyncAction::Download,
            (Some(_), None) => SyncAction::Upload,
            (None, None) => SyncAction::Nothing,
            (Some(l), Some(r)) if l == r => SyncAction::Nothing,
            (Some(_), Some(_)) => {
                let changed_local = self.local_fp != self.last_fp;
                let changed_remote = self.remote_fp != self.last_fp;

                match (changed_local, changed_remote) {
                    (false, true) => SyncAction::Download,
                    (true, false) => SyncAction::Upload,
                    (false, false) => SyncAction::Nothing,
                    (true, true) => SyncAction::Conflict,
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum CtrArchiveKind {
    Savedata,
    Extdata,
}

impl Display for CtrArchiveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CtrArchiveKind::Savedata => write!(f, "save"),
            CtrArchiveKind::Extdata => write!(f, "extdata"),
        }
    }
}

impl TryFrom<&str> for CtrArchiveKind {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "save" => Ok(CtrArchiveKind::Savedata),
            "extdata" => Ok(CtrArchiveKind::Extdata),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SyncAction {
    Nothing,
    Conflict,
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
    fn no_change() {
        let s = fixture();

        assert!(matches!(s.get_action(), SyncAction::Nothing))
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
    fn no_remote_no_local_always_no_action() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: None,
            remote_fp: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Nothing));
    }
    #[test]
    fn matching_local_and_remote_always_no_action() {
        let s = SyncState {
            last_fp: Some(1),
            local_fp: Some(2),
            remote_fp: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Nothing));
    }

    fn fixture() -> SyncState {
        SyncState {
            title_id: 0x00040000_1234ABCD,
            product_code: "XTR-X-ABCD".into(),
            archive_kind: CtrArchiveKind::Savedata,
            last_fp: None,
            local_fp: None,
            remote_fp: None,
        }
    }
}
