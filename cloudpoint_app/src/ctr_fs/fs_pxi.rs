use super::{CtrContext, CtrLeaf};
use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use std::io::{self, Cursor};
use std::str::FromStr;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(super) mod driver;

/// Archive type only really supports a single GBA save at a fixed binary path,
/// so only one fake leaf is supported and paths passed in are just ignored
static AGB_FAKEPATH: &str = "agb_save.bin";

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct FsPxiLeaf;

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct FsPxiContext {
    pub(in crate::ctr_fs) archive: driver::FsPxiArchive,
}

impl Leaf for FsPxiLeaf {
    type Context = FsPxiContext;

    fn new(_path: impl AsRef<Path>, _ctx: &Self::Context) -> Result<Self, TreeError> {
        log::debug!("creating leaf for gba save using fake path {AGB_FAKEPATH}");

        Ok(Self)
    }

    fn delete(&mut self, _ctx: &Self::Context) -> Result<(), TreeError> {
        log::debug!("deleting is unsupported for gba save");

        Err(io::Error::from(io::ErrorKind::Unsupported).into())
    }

    fn path(&self, _ctx: &Self::Context) -> &Path {
        Path::new(&AGB_FAKEPATH)
    }

    fn data(&self, ctx: &Self::Context) -> Result<impl io::Read + io::Seek, TreeError> {
        Ok(Cursor::new(ctx.archive.buffer_to_vec()))
    }

    fn len(&self, ctx: &Self::Context) -> Result<u64, TreeError> {
        Ok(ctx.archive.buffer_len() as u64)
    }

    fn set_len(&mut self, length: u64, ctx: &Self::Context) -> Result<(), TreeError> {
        log::debug!(
            "resizing is unsupported for gba save, ensuring requested size already matches"
        );

        match self.len(ctx)? == length {
            true => Ok(()),
            false => Err(io::Error::from(io::ErrorKind::Unsupported).into()),
        }
    }

    fn write_chunk(
        &mut self,
        offset: u64,
        source: &mut impl io::Read,
        ctx: &Self::Context,
    ) -> Result<(), TreeError> {
        log::debug!("writing chunk for gba save");

        ctx.archive.buffer_write(offset, source)?;

        Ok(())
    }
}

pub fn from_archive(archive: driver::FsPxiArchive) -> Result<Tree<CtrLeaf>> {
    log::debug!(
        "creating synthetic local tree for fs_pxi archive {}",
        archive.sync_item()
    );

    let ctx = FsPxiContext { archive };
    let item: HashMap<PathBuf, CtrLeaf> = HashMap::from_iter(
        [(
            PathBuf::from_str(AGB_FAKEPATH)?,
            CtrLeaf::FsPxi(FsPxiLeaf::new(AGB_FAKEPATH, &ctx)?),
        )]
        .into_iter(),
    );

    Ok(Tree::new(item, CtrContext::FsPxi(ctx)))
}
