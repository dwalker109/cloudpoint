use crate::ffi::{
    FS_ATTRIBUTE_DIRECTORY, FS_OPEN_READ, FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, PATH_ASCII,
    PATH_BINARY, ctr_close_archive, ctr_close_directory, ctr_close_file, ctr_commit_archive,
    ctr_create_directory, ctr_create_file, ctr_delete_file, ctr_get_file_size,
    ctr_getr_ext_data_id_for_title, ctr_open_archive, ctr_open_directory, ctr_open_file,
    ctr_read_directory, ctr_read_file, ctr_reset_secure_save_meta, ctr_set_file_size,
    ctr_write_file, fsMakePath,
};
use anyhow::Result;
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::sync::CtrArchiveKind;
use ctru::services::fs::{ArchiveID, MediaType};
use std::{
    collections::HashMap,
    ffi::c_void,
    io::{self, Cursor},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CtrArchive {
    pub title_id: u64,
    pub kind: CtrArchiveKind,
    pub handle: u64,
}

impl CtrArchive {
    pub fn open(title_id: u64, kind: CtrArchiveKind) -> Result<Self> {
        let path = CtrArchivePath::new(title_id, kind)?;
        let handle = ctr_open_archive(&path)?;

        Ok(Self {
            title_id,
            kind,
            handle,
        })
    }
}

impl Drop for CtrArchive {
    fn drop(&mut self) {
        if self.kind == CtrArchiveKind::Savedata {
            ctr_commit_archive(&self).expect("save archive committed");
            ctr_reset_secure_save_meta(self.title_id).expect("secure save meta reset");
        }

        ctr_close_archive(&self).expect("archive closed");
    }
}

pub struct CtrArchivePath {
    pub title_id: u64,
    buffer: [u32; 3],
    pub archive_id: ArchiveID,
}

impl CtrArchivePath {
    pub fn new(title_id: u64, kind: CtrArchiveKind) -> Result<Self> {
        let (buffer, archive_id) = match kind {
            CtrArchiveKind::Savedata => (
                [
                    MediaType::Sd as u32,
                    title_id as u32,
                    (title_id >> 32) as u32,
                ],
                ArchiveID::UserSavedata,
            ),
            CtrArchiveKind::Extdata => {
                let extdata_id = ctr_getr_ext_data_id_for_title(title_id)?;

                (
                    [MediaType::Sd as u32, extdata_id as u32, 0],
                    ArchiveID::Extdata,
                )
            }
        };

        Ok(Self {
            title_id,
            buffer,
            archive_id,
        })
    }

    pub fn fs_path(&self) -> FS_Path {
        FS_Path {
            type_: PATH_BINARY,
            size: 12,
            data: self.buffer.as_ptr() as *const c_void,
        }
    }
}

pub struct CtrFilePath(pub String);

impl CtrFilePath {
    pub fn new(path: &str) -> Self {
        Self(path.into())
    }

    pub fn fs_path(&self) -> FS_Path {
        unsafe { fsMakePath(PATH_ASCII, self.0.as_ptr() as *const _) }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct CtrArchiveLeaf {
    path: String,
    ctx: Arc<CtrArchive>,
}

impl Leaf for CtrArchiveLeaf {
    type Context = Arc<CtrArchive>;

    fn new(path: impl AsRef<Path>, ctx: Self::Context) -> Result<Self, TreeError> {
        let path = path.as_ref().to_string_lossy().into_owned();

        Ok(Self {
            path,
            ctx: ctx.clone(),
        })
    }

    fn pad(&mut self, length: u64) -> Result<(), TreeError> {
        let path = CtrFilePath::new(&self.path);

        match ctr_open_file(&self.ctx, &path, FS_OPEN_READ) {
            // File exists, check size and resize if needed
            Ok(handle) => {
                let curr_length = ctr_get_file_size(handle)?;
                ctr_close_file(handle)?;

                if curr_length != length {
                    match self.ctx.kind {
                        // Savedata supports resize in place
                        CtrArchiveKind::Savedata => {
                            let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_WRITE)?;
                            ctr_set_file_size(handle, length)?;
                            ctr_close_file(handle)?;
                        }
                        // Extdata requires recreating the file with a new length, resizes aren't supported
                        CtrArchiveKind::Extdata => {
                            let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_READ)?;
                            let mut buffer = ctr_read_file(handle, 0, curr_length)?;
                            buffer.resize(length as usize, 0x00);
                            ctr_close_file(handle)?;

                            ctr_delete_file(&self.ctx, &path)?;
                            ctr_create_file(&self.ctx, &path, length)?;

                            let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_WRITE)?;
                            ctr_write_file(handle, 0, &buffer, FS_WRITE_FLUSH)?;
                            ctr_close_file(handle)?;
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
                    let dir_path = CtrFilePath::new(&format!("{}\0", &self.path[0..=sep]));

                    if let Ok(h) = ctr_open_directory(&self.ctx, &dir_path) {
                        ctr_close_directory(h)?;
                    } else {
                        ctr_create_directory(&self.ctx, &dir_path)?;
                    }
                }

                ctr_create_file(&self.ctx, &path, length)?;
            }
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), TreeError> {
        let path = CtrFilePath::new(&self.path);
        ctr_delete_file(&self.ctx, &path)?;

        Ok(())
    }

    fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    fn data(&self) -> Result<impl io::Read + io::Seek, TreeError> {
        let path = CtrFilePath::new(&self.path);
        let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_READ)?;
        let file_size = ctr_get_file_size(handle)?;
        let data = ctr_read_file(handle, 0, file_size)?;
        ctr_close_file(handle)?;

        Ok(Cursor::new(data))
    }

    fn length(&self) -> Result<u64, TreeError> {
        let path = CtrFilePath::new(&self.path);
        let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_READ)?;
        let file_size = ctr_get_file_size(handle)?;
        ctr_close_file(handle)?;

        Ok(file_size)
    }

    fn write_chunk(&mut self, offset: u64, source: &mut impl io::Read) -> Result<(), TreeError> {
        let mut buf = Vec::new();
        source.read_to_end(&mut buf)?;

        let path = CtrFilePath::new(&self.path);
        let handle = ctr_open_file(&self.ctx, &path, FS_OPEN_WRITE)?;
        ctr_write_file(handle, offset, &buf, FS_WRITE_FLUSH)?;
        ctr_close_file(handle)?;

        Ok(())
    }
}

pub fn walk_tree(title_id: u64, mode: CtrArchiveKind) -> Result<Tree<CtrArchiveLeaf>> {
    let ctx = Arc::new(CtrArchive::open(title_id, mode)?);
    let mut results = HashMap::new();

    walk_sub("/".into(), &ctx, &mut results)?;

    fn walk_sub(
        dir_path: String,
        ctx: &<CtrArchiveLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, CtrArchiveLeaf>,
    ) -> Result<()> {
        let with_null = format!("{dir_path}\0");
        let path = CtrFilePath::new(&with_null);
        let handle = ctr_open_directory(&ctx, &path)?;
        let entries = ctr_read_directory(handle)?;

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
