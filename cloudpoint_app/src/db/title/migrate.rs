use super::TitleDb as TitleDbV1;
use crate::db::TitleDetails;
use cloudpoint_lib::db::{Migrate, MigrateRoot};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;

impl Migrate for TitleDbV1 {
    const VERSION_ID: u16 = 1;

    type Previous = TitleDbV0;

    fn migrate(prev: Self::Previous) -> anyhow::Result<Self> {
        let TitleDbV0(_path, items, installed) = prev;

        Ok(Self(items, installed))
    }
}

#[derive(Deserialize)]
pub struct TitleDbV0(
    #[serde(skip)] PathBuf,
    HashMap<u64, TitleDetails>,
    BTreeSet<u64>,
);

impl Migrate for TitleDbV0 {
    const VERSION_ID: u16 = 0;

    type Previous = MigrateRoot;

    fn migrate(_prev: Self::Previous) -> anyhow::Result<Self> {
        unreachable!()
    }
}
