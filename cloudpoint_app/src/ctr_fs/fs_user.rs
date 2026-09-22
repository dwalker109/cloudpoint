use super::{CtrContext, CtrLeaf};
use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::sync::SyncItem;
use ctru_sys::{FS_ATTRIBUTE_DIRECTORY, FS_OPEN_READ, FS_OPEN_WRITE, FS_WRITE_FLUSH};
use std::io;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub(super) mod driver;

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct FsUserLeaf {
    path: String,
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct FsUserContext {
    pub(in crate::ctr_fs) archive: driver::FsUserArchive,
}

impl Leaf for FsUserLeaf {
    type Context = FsUserContext;

    fn new(path: impl AsRef<Path>, _ctx: &Self::Context) -> Result<Self, TreeError> {
        let path = path.as_ref().to_string_lossy().into_owned();

        Ok(Self { path })
    }

    fn delete(&mut self, ctx: &Self::Context) -> Result<(), TreeError> {
        log::debug!("deleting {}", &self.path);

        let path = driver::FsUserInnerPath::new(&self.path)?;
        ctx.archive.delete_file(&path)?;

        Ok(())
    }

    fn path(&self, _ctx: &Self::Context) -> &Path {
        Path::new(&self.path)
    }

    fn data(&self, ctx: &Self::Context) -> Result<impl io::Read + io::Seek, TreeError> {
        let path = driver::FsUserInnerPath::new(&self.path)?;
        let file = ctx.archive.open_file(&path, FS_OPEN_READ)?;
        let reader = io::BufReader::with_capacity(256 * 1024, file.into_reader()?);

        Ok(reader)
    }

    fn len(&self, ctx: &Self::Context) -> Result<u64, TreeError> {
        let path = driver::FsUserInnerPath::new(&self.path)?;
        let file = ctx.archive.open_file(&path, FS_OPEN_READ)?;
        let file_size = file.size()?;

        Ok(file_size)
    }

    fn set_len(&mut self, length: u64, ctx: &Self::Context) -> Result<(), TreeError> {
        let path = driver::FsUserInnerPath::new(&self.path)?;

        match ctx.archive.open_file(&path, FS_OPEN_READ) {
            // File exists, check size and resize if needed
            Ok(file) => {
                let curr_size = file.size()?;
                drop(file);

                if curr_size != length {
                    match ctx.archive.sync_item() {
                        // Savedata supports resize in place
                        SyncItem::Savedata(_) => {
                            log::debug!("setting length for {} via syscall", &self.path);

                            let file = ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
                            file.set_size(length)?;
                        }
                        // Extdata requires recreating the file with a new length, resizes aren't supported
                        SyncItem::Extdata(_) => {
                            log::debug!("setting length for {} via recreate", &self.path);

                            let file = ctx.archive.open_file(&path, FS_OPEN_READ)?;
                            let mut buffer = file.into_reader()?.read_to_vec(0, curr_size)?;
                            buffer.resize(length as usize, 0x00);

                            ctx.archive.delete_file(&path)?;
                            ctx.archive.create_file(&path, length)?;

                            let file = ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
                            file.write(0, &buffer, FS_WRITE_FLUSH)?;
                        }
                    }
                }
            }
            // Probably doesn't exist, try to create it (including intermediary directories)
            Err(_) => {
                log::debug!("setting length for {} via initial create", &self.path);

                let path_separators = self
                    .path
                    .char_indices()
                    .filter_map(|(i, c)| (c == '/').then_some(i))
                    .skip(1)
                    .collect::<Vec<_>>();

                for sep in path_separators {
                    let dir_path = driver::FsUserInnerPath::new(&self.path[0..=sep])?;

                    if let Err(_) = ctx.archive.open_directory(&dir_path) {
                        ctx.archive.create_directory(&dir_path)?;
                    }
                }

                ctx.archive.create_file(&path, length)?;
            }
        }

        Ok(())
    }

    fn write_chunk(
        &mut self,
        offset: u64,
        source: &mut impl io::Read,
        ctx: &Self::Context,
    ) -> Result<(), TreeError> {
        log::debug!("writing chunk for {}", &self.path);

        let mut buf = Vec::new();
        source.read_to_end(&mut buf)?;

        let path = driver::FsUserInnerPath::new(&self.path)?;
        let file = ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
        file.write(offset, &buf, FS_WRITE_FLUSH)?;

        Ok(())
    }
}

pub fn from_archive(archive: driver::FsUserArchive) -> Result<Tree<CtrLeaf>> {
    log::debug!("creating local tree for fs_user archive {:?}", archive);

    let ctx = FsUserContext { archive };
    let mut results = HashMap::new();

    walk_sub("/", &ctx, &mut results)?;

    fn walk_sub(
        dir_path: &str,
        ctx: &<FsUserLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, CtrLeaf>,
    ) -> Result<()> {
        log::debug!("checking {dir_path}");

        let path = driver::FsUserInnerPath::new(dir_path)?;
        let directory = ctx.archive.open_directory(&path)?;
        let entries = directory.read()?;

        for entry in entries {
            let mut fq_path = [
                dir_path,
                String::from_utf16(&entry.name)?.trim_end_matches('\0'),
            ]
            .join("");

            if entry.attributes & FS_ATTRIBUTE_DIRECTORY != 0 {
                log::debug!("found subdir, descending");
                fq_path.push('/');
                walk_sub(&fq_path, ctx, results)?;
            } else {
                log::debug!("found file, adding");
                let fq_path = PathBuf::from(fq_path);
                let leaf = FsUserLeaf::new(&fq_path, ctx)?;
                results.insert(fq_path, CtrLeaf::FsUser(leaf));
            }
        }

        Ok(())
    }

    let tree = Tree::new(results, CtrContext::FsUser(ctx));

    Ok(tree)
}
