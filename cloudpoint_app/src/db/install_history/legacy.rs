use super::InstallHistoryDb;
use cloudpoint_lib::sync::SyncItem;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

pub use v0::InstallHistoryDbV0;

mod v0 {

    use super::*;

    #[derive(Deserialize)]
    pub struct InstallHistoryDbV0(#[serde(skip)] PathBuf, HashMap<(u64, SyncItem), u64>);

    impl From<InstallHistoryDbV0> for InstallHistoryDb {
        fn from(legacy: InstallHistoryDbV0) -> Self {
            let InstallHistoryDbV0(_path, items) = legacy;

            Self(items)
        }
    }
}
