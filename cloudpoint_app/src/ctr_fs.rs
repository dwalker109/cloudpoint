use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::{ctr::CtrSmdh, sync::SyncItem};
use std::{
    io::{Error as IoError, Read, Seek},
    path::Path,
};

mod fs_user;
mod fs_pxi {}

pub fn smdh(sync_item: SyncItem) -> Result<CtrSmdh, IoError> {
    match sync_item {
        SyncItem::Savedata(_) | SyncItem::Extdata(_) => {
            fs_user::driver::FsUserArchive::smdh(sync_item)
        }
    }
}

pub enum CtrArchive {
    FsUser(fs_user::driver::FsUserArchive),
}

impl CtrArchive {
    pub fn open(sync_item: SyncItem) -> Result<Self, IoError> {
        match sync_item {
            SyncItem::Savedata(_) | SyncItem::Extdata(_) => Ok(CtrArchive::FsUser(
                fs_user::driver::FsUserArchive::open(sync_item)?,
            )),
        }
    }

    pub fn into_tree(self) -> Result<Tree<CtrLeaf>> {
        match self {
            CtrArchive::FsUser(archive) => fs_user::from_archive(archive),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum CtrLeaf {
    FsUser(fs_user::FsUserLeaf),
}

pub enum CtrContext {
    FsUser(fs_user::FsUserContext),
}

impl CtrContext {
    pub fn finalise(&self) -> std::io::Result<()> {
        match self {
            CtrContext::FsUser(ctx) => ctx.archive.finalise(),
        }
    }
}

impl Leaf for CtrLeaf {
    type Context = CtrContext;

    fn new(path: impl AsRef<Path>, ctx: &Self::Context) -> Result<Self, TreeError> {
        match ctx {
            CtrContext::FsUser(ctx) => Ok(Self::FsUser(fs_user::FsUserLeaf::new(path, ctx)?)),
        }
    }

    fn delete(&mut self, ctx: &Self::Context) -> Result<(), TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.delete(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn path(&self, ctx: &Self::Context) -> &Path {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.path(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn data(&self, ctx: &Self::Context) -> Result<impl Read + Seek, TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.data(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn len(&self, ctx: &Self::Context) -> Result<u64, TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.len(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn set_len(&mut self, length: u64, ctx: &Self::Context) -> Result<(), TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.set_len(length, ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn write_chunk(
        &mut self,
        offset: u64,
        source: &mut impl std::io::prelude::Read,
        ctx: &Self::Context,
    ) -> Result<(), TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => {
                leaf.write_chunk(offset, source, ctx)
            }
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }
}
