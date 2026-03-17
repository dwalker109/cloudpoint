#[derive(Debug, Clone)]
struct SyncState {
    title_id: u64,
    product_code: String,
    fp_loc_last: Option<u64>,
    fp_loc_curr: Option<u64>,
    fp_rem_last: Option<u64>,
    fp_rem_curr: Option<u64>,
}

impl SyncState {
    pub fn set_local_fp(&mut self, fp: u64) {
        self.fp_loc_curr = Some(fp);
    }

    pub fn set_remote_fp(&mut self, fp: u64) {
        self.fp_rem_curr = Some(fp);
    }

    pub fn get_action(&self) -> SyncAction {
        match (self.fp_loc_curr, self.fp_rem_curr) {
            (None, Some(_)) => SyncAction::Download,
            (Some(_), None) => SyncAction::Upload,
            (None, None) => SyncAction::Nothing,
            (Some(_), Some(_)) => {
                let changed_local = self.fp_loc_curr != self.fp_loc_last;
                let changed_remote = self.fp_rem_curr != self.fp_rem_last;

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
enum SyncAction {
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
        s.set_local_fp(1);

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_only_no_local() {
        let mut s = SyncState { ..fixture() };
        s.set_remote_fp(1);

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
            fp_loc_last: Some(1),
            fp_loc_curr: Some(2),
            fp_rem_last: Some(1),
            fp_rem_curr: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn remote_change_only() {
        let s = SyncState {
            fp_loc_last: Some(1),
            fp_loc_curr: Some(1),
            fp_rem_last: Some(1),
            fp_rem_curr: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn both_change() {
        let s = SyncState {
            fp_loc_last: Some(1),
            fp_loc_curr: Some(2),
            fp_rem_last: Some(1),
            fp_rem_curr: Some(2),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Conflict));
    }

    #[test]
    fn no_local_with_remote_always_download() {
        let s = SyncState {
            fp_loc_last: Some(1),
            fp_loc_curr: None,
            fp_rem_last: Some(1),
            fp_rem_curr: Some(1),
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Download));
    }

    #[test]
    fn no_remote_with_local_always_upload() {
        let s = SyncState {
            fp_loc_last: Some(1),
            fp_loc_curr: Some(1),
            fp_rem_last: Some(1),
            fp_rem_curr: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Upload));
    }

    #[test]
    fn no_remote_no_local_always_no_action() {
        let s = SyncState {
            fp_loc_last: Some(1),
            fp_loc_curr: None,
            fp_rem_last: Some(1),
            fp_rem_curr: None,
            ..fixture()
        };

        let res = s.get_action();

        assert!(matches!(res, SyncAction::Nothing));
    }

    fn fixture() -> SyncState {
        SyncState {
            title_id: 0x00040000_1234ABCD,
            product_code: "XTR-X-ABCD".into(),
            fp_loc_last: None,
            fp_loc_curr: None,
            fp_rem_last: None,
            fp_rem_curr: None,
        }
    }
}
