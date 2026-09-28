use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::{ctr::CtrSmdh, sync::SyncItem};
use std::{
    io::{Error as IoError, Read, Seek},
    path::Path,
};

mod fs_pxi;
mod fs_user;

pub fn smdh(sync_item: SyncItem) -> Result<CtrSmdh, IoError> {
    match sync_item {
        SyncItem::Savedata(_) | SyncItem::Extdata(_) => {
            fs_user::driver::FsUserArchive::smdh(sync_item)
        }
        SyncItem::Gba(title_id) => {
            log::debug!("smdh for gba item satisfied by synthetic savedata item");
            fs_user::driver::FsUserArchive::smdh(SyncItem::Savedata(title_id))
        }
    }
}

pub enum CtrArchive {
    FsUser(fs_user::driver::FsUserArchive),
    FsPxi(fs_pxi::driver::FsPxiArchive),
}

impl CtrArchive {
    pub fn open(sync_item: SyncItem) -> Result<Self, IoError> {
        match sync_item {
            SyncItem::Savedata(_) | SyncItem::Extdata(_) => Ok(CtrArchive::FsUser(
                fs_user::driver::FsUserArchive::open(sync_item)?,
            )),
            SyncItem::Gba(_) => Ok(CtrArchive::FsPxi(fs_pxi::driver::FsPxiArchive::open(
                sync_item,
            )?)),
        }
    }

    pub fn into_tree(self) -> Result<Tree<CtrLeaf>> {
        match self {
            CtrArchive::FsUser(archive) => fs_user::from_archive(archive),
            CtrArchive::FsPxi(archive) => fs_pxi::from_archive(archive),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum CtrLeaf {
    FsUser(fs_user::FsUserLeaf),
    FsPxi(fs_pxi::FsPxiLeaf),
}

pub enum CtrContext {
    FsUser(fs_user::FsUserContext),
    FsPxi(fs_pxi::FsPxiContext),
}

impl CtrContext {
    pub fn finalise(&self) -> std::io::Result<()> {
        match self {
            CtrContext::FsUser(ctx) => ctx.archive.finalise(),
            CtrContext::FsPxi(ctx) => ctx.archive.finalise(),
        }
    }
}

pub enum CtrReader<U, P> {
    FsUser(U),
    FsPxi(P),
}

impl<U: Read, P: Read> Read for CtrReader<U, P> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            CtrReader::FsUser(r) => r.read(buf),
            CtrReader::FsPxi(r) => r.read(buf),
        }
    }
}

impl<U: Seek, P: Seek> Seek for CtrReader<U, P> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            CtrReader::FsUser(s) => s.seek(pos),
            CtrReader::FsPxi(s) => s.seek(pos),
        }
    }
}

impl Leaf for CtrLeaf {
    type Context = CtrContext;

    fn new(path: impl AsRef<Path>, ctx: &Self::Context) -> Result<Self, TreeError> {
        Ok(match ctx {
            CtrContext::FsUser(ctx) => Self::FsUser(fs_user::FsUserLeaf::new(path, ctx)?),
            CtrContext::FsPxi(ctx) => Self::FsPxi(fs_pxi::FsPxiLeaf::new(path, ctx)?),
        })
    }

    fn delete(&mut self, ctx: &Self::Context) -> Result<(), TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.delete(ctx),
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => leaf.delete(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn path(&self, ctx: &Self::Context) -> &Path {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.path(ctx),
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => leaf.path(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn data(&self, ctx: &Self::Context) -> Result<impl Read + Seek, TreeError> {
        Ok(match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => CtrReader::FsUser(leaf.data(ctx)?),
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => CtrReader::FsPxi(leaf.data(ctx)?),
            _ => unreachable!("leaf & context associated type mismatch"),
        })
    }

    fn len(&self, ctx: &Self::Context) -> Result<u64, TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.len(ctx),
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => leaf.len(ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }

    fn set_len(&mut self, length: u64, ctx: &Self::Context) -> Result<(), TreeError> {
        match (self, ctx) {
            (CtrLeaf::FsUser(leaf), CtrContext::FsUser(ctx)) => leaf.set_len(length, ctx),
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => leaf.set_len(length, ctx),
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
            (CtrLeaf::FsPxi(leaf), CtrContext::FsPxi(ctx)) => leaf.write_chunk(offset, source, ctx),
            _ => unreachable!("leaf & context associated type mismatch"),
        }
    }
}
