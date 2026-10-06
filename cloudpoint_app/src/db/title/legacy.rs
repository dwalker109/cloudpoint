use super::TitleDb;
use crate::db::TitleDetails;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;

pub use v0::TitleDbV0;

mod v0 {

    use super::*;

    #[derive(Deserialize)]
    pub struct TitleDbV0(
        #[serde(skip)] PathBuf,
        HashMap<u64, TitleDetails>,
        BTreeSet<u64>,
    );

    impl From<TitleDbV0> for TitleDb {
        fn from(legacy: TitleDbV0) -> Self {
            let TitleDbV0(_path, items, installed) = legacy;

            Self(items, installed)
        }
    }
}
