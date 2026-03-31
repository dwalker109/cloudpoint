use crate::ctr_fs::{CtrArchive, CtrFsPath};

use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::sync::CtrArchiveKind;
use ctru_sys::{FS_ATTRIBUTE_DIRECTORY, FS_OPEN_READ, FS_OPEN_WRITE, FS_WRITE_FLUSH};
use std::io::{self, Cursor};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct CtrArchiveLeaf {
    path: String,
    ctx: CtrArchiveLeafContext,
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct CtrArchiveLeafContext {
    archive: Arc<CtrArchive>,
}

impl Leaf for CtrArchiveLeaf {
    type Context = CtrArchiveLeafContext;

    fn new(path: impl AsRef<Path>, ctx: Self::Context) -> Result<Self, TreeError> {
        let path = path.as_ref().to_string_lossy().into_owned();

        Ok(Self {
            path,
            ctx: ctx.clone(),
        })
    }

    fn pad(&mut self, length: u64) -> Result<(), TreeError> {
        let path = CtrFsPath::new(&self.path);

        match self.ctx.archive.open_file(&path, FS_OPEN_READ) {
            // File exists, check size and resize if needed
            Ok(file) => {
                let curr_size = file.size()?;
                drop(file);

                if curr_size != length {
                    match self.ctx.archive.kind() {
                        // Savedata supports resize in place
                        CtrArchiveKind::Savedata => {
                            let file = self.ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
                            file.set_size(length)?;
                        }
                        // Extdata requires recreating the file with a new length, resizes aren't supported
                        CtrArchiveKind::Extdata => {
                            let file = self.ctx.archive.open_file(&path, FS_OPEN_READ)?;
                            let mut buffer = file.read(0, curr_size)?;
                            buffer.resize(length as usize, 0x00);
                            drop(file);

                            self.ctx.archive.delete_file(&path)?;
                            self.ctx.archive.create_file(&path, length)?;

                            let file = self.ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
                            file.write(0, &buffer, FS_WRITE_FLUSH)?;
                        }
                    }
                }
            }
            // Probably doesn't exist, try to create it (including intermediary directories)
            Err(_) => {
                let path_separators = self
                    .path
                    .char_indices()
                    .filter_map(|(i, c)| (c == '/').then_some(i))
                    .skip(1)
                    .collect::<Vec<_>>();

                for sep in path_separators {
                    let dir_path = CtrFsPath::new(&format!("{}\0", &self.path[0..=sep]));

                    if let Err(_) = self.ctx.archive.open_directory(&dir_path) {
                        self.ctx.archive.create_directory(&dir_path)?;
                    }
                }

                self.ctx.archive.create_file(&path, length)?;
            }
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), TreeError> {
        let path = CtrFsPath::new(&self.path);
        self.ctx.archive.delete_file(&path)?;

        Ok(())
    }

    fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    fn data(&self) -> Result<impl io::Read + io::Seek, TreeError> {
        let path = CtrFsPath::new(&self.path);
        let file = self.ctx.archive.open_file(&path, FS_OPEN_READ)?;
        let file_size = file.size()?;
        let data = file.read(0, file_size)?;

        Ok(Cursor::new(data))
    }

    fn length(&self) -> Result<u64, TreeError> {
        let path = CtrFsPath::new(&self.path);
        let file = self.ctx.archive.open_file(&path, FS_OPEN_READ)?;
        let file_size = file.size()?;

        Ok(file_size)
    }

    fn write_chunk(&mut self, offset: u64, source: &mut impl io::Read) -> Result<(), TreeError> {
        let mut buf = Vec::new();
        source.read_to_end(&mut buf)?;

        let path = CtrFsPath::new(&self.path);
        let file = self.ctx.archive.open_file(&path, FS_OPEN_WRITE)?;
        file.write(offset, &buf, FS_WRITE_FLUSH)?;

        Ok(())
    }
}

pub fn from_archive(archive: Arc<CtrArchive>) -> Result<Tree<CtrArchiveLeaf>> {
    let ctx = CtrArchiveLeafContext { archive };
    let mut results = HashMap::new();

    walk_sub("/".into(), &ctx, &mut results)?;

    fn walk_sub(
        dir_path: String,
        ctx: &<CtrArchiveLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, CtrArchiveLeaf>,
    ) -> Result<()> {
        let with_null = format!("{dir_path}\0");
        let path = CtrFsPath::new(&with_null);
        let directory = ctx.archive.open_directory(&path)?;
        let entries = directory.read()?;

        for entry in entries {
            let fq_path = format!(
                "{dir_path}{}",
                String::from_utf16_lossy(&entry.name).trim_end_matches('\0')
            );

            if entry.attributes & FS_ATTRIBUTE_DIRECTORY != 0 {
                walk_sub(format!("{fq_path}/"), ctx, results)?;
            } else {
                let fq_path = PathBuf::from(format!("{fq_path}\0"));
                let leaf = CtrArchiveLeaf::new(&fq_path, ctx.clone()).expect("leaf created");
                results.insert(fq_path, leaf);
            }
        }

        Ok(())
    }

    Ok(Tree::new(results, ctx))
}
