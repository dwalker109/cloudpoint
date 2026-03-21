use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub title_id: u64,
    pub product_code: String,
    pub fingerprint_local_last: Option<u64>,
    pub fingerprint_remote_last: Option<u64>,
    #[serde(skip)]
    pub fingerprint_local_curr: Option<u64>,
    #[serde(skip)]
    pub fingerprint_remote_curr: Option<u64>,
}

impl SyncState {
    pub fn get_action(&self) -> SyncAction {
        match (self.fingerprint_local_curr, self.fingerprint_remote_curr) {
            (None, Some(_)) => SyncAction::Download,
            (Some(_), None) => SyncAction::Upload,
            (None, None) => SyncAction::Nothing,
            (Some(_), Some(_)) => {
                let changed_local = self.fingerprint_local_curr != self.fingerprint_local_last;
                let changed_remote = self.fingerprint_remote_curr != self.fingerprint_remote_last;

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
        s.fingerprint_local_curr = Some(1);

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_only_no_local() {
        let mut s = SyncState { ..fixture() };
        s.fingerprint_remote_curr = Some(1);

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
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: Some(2),
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_change_only() {
        let s = SyncState {
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: Some(1),
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn both_change() {
        let s = SyncState {
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: Some(2),
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Conflict));
    }

    #[test]
    fn no_local_with_remote_always_download() {
        let s = SyncState {
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: None,
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn no_remote_with_local_always_upload() {
        let s = SyncState {
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: Some(1),
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn no_remote_no_local_always_no_action() {
        let s = SyncState {
            fingerprint_local_last: Some(1),
            fingerprint_local_curr: None,
            fingerprint_remote_last: Some(1),
            fingerprint_remote_curr: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Nothing));
    }

    fn fixture() -> SyncState {
        SyncState {
            title_id: 0x00040000_1234ABCD,
            product_code: "XTR-X-ABCD".into(),
            fingerprint_local_last: None,
            fingerprint_local_curr: None,
            fingerprint_remote_last: None,
            fingerprint_remote_curr: None,
        }
    }
}
