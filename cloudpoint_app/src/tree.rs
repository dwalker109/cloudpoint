use anyhow::Result;
use chunktree::tree::{Leaf, TreeError};
use std::{
    io::{Read, Seek},
    path::Path,
};

pub mod fs_user;

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
