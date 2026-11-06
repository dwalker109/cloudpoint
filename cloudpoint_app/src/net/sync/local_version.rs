use crate::{
    ctr_fs::{CtrArchive, CtrLeaf},
    title::meta,
};
use anyhow::Result;
use chunktree::{
    tree::Tree,
    version::{ChunkStrategy, Concurrency, Version},
};
use cloudpoint_lib::{ctr::CtrMeta, sync::SyncItem};

pub struct LocalVersionInfo {
    meta: CtrMeta,
    tree: Tree<CtrLeaf>,
    ver: Version<CtrLeaf, CtrMeta>,
}

impl LocalVersionInfo {
    pub fn try_new(sync_item: SyncItem) -> Result<Self> {
        let meta = meta(sync_item)?;
        let tree = CtrArchive::open(sync_item)?.into_tree()?;
        let ver = Version::new(
            &tree,
            meta,
            ChunkStrategy::FixedSize(256 * 1024),
            Concurrency::Serial,
        )?;

        Ok(Self { meta, tree, ver })
    }

    pub fn fingerprint(&self) -> Option<u128> {
        Some(self.ver.fingerprint())
    }

    pub fn into_parts(self) -> Result<(CtrMeta, Tree<CtrLeaf>, Version<CtrLeaf, CtrMeta>)> {
        let Self { meta, tree, ver } = self;

        Ok((meta, tree, ver))
    }
}
