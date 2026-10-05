use super::InstallHistoryDb;
use cloudpoint_lib::sync::SyncItem;
use serde::Deserialize;
use std::collections::HashMap;

pub use v0::InstallHistoryDbV0;

mod v0 {
    use super::*;

    #[derive(Deserialize)]
    pub struct InstallHistoryDbV0(HashMap<(u64, SyncItem), u64>);

    impl From<InstallHistoryDbV0> for InstallHistoryDb {
        fn from(legacy: InstallHistoryDbV0) -> Self {
            Self(legacy.0)
        }
    }
}
