use anyhow::{Result, anyhow, bail};
use chunktree::tree::{Leaf, Tree, TreeError};
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    ARCHIVE_ACTION_COMMIT_SAVE_DATA, FS_ATTRIBUTE_DIRECTORY, FS_Archive, FS_DirectoryEntry,
    FS_OPEN_CREATE, FS_OPEN_READ, FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, FSDIR_Read, FSFILE_Close,
    FSFILE_GetSize, FSFILE_Read, FSFILE_SetSize, FSFILE_Write, FSUSER_CloseArchive,
    FSUSER_ControlArchive, FSUSER_DeleteFile, FSUSER_OpenArchive, FSUSER_OpenDirectory,
    FSUSER_OpenFile, PATH_ASCII, PATH_BINARY, PATH_UTF16, R_FAILED, fsInit, fsMakePath,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::c_void,
    io::{self, Cursor},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CtruUserSaveArchive {
    pub title_id: u64,
    pub archive: FS_Archive,
}

impl CtruUserSaveArchive {
    pub fn open(title_id: u64) -> Result<Self> {
        let mut archive: FS_Archive = 0;

        unsafe {
            fsInit();

            let data: [u32; 3] = [
                MediaType::Sd as u32,
                title_id as u32,
                (title_id >> 32) as u32,
            ];

            let path = FS_Path {
                type_: PATH_BINARY,
                size: (data.len() * 4) as u32,
                data: data.as_ptr() as *const c_void,
            };

            let res = FSUSER_OpenArchive(&mut archive, ArchiveID::UserSavedata as u32, path);

            if R_FAILED(res) {
                bail!("Could not open archive for {}", title_id);
            }
        }

        Ok(Self { title_id, archive })
    }
}

impl Drop for CtruUserSaveArchive {
    fn drop(&mut self) {
        unsafe {
            let res = FSUSER_ControlArchive(
                self.archive,
                ARCHIVE_ACTION_COMMIT_SAVE_DATA,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            );

            if R_FAILED(res) {
                panic!(
                    "failed to commit archive {} for title {}",
                    self.archive, self.title_id
                );
            }

            let res = FSUSER_CloseArchive(self.archive);

            if R_FAILED(res) {
                panic!("Could not close archive for {}", self.title_id)
            }
        }
    }
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq)]
pub struct ArchiveFileLeaf {
    path: String,
    ctx: Arc<CtruUserSaveArchive>,
}

impl Leaf for ArchiveFileLeaf {
    type Context = Arc<CtruUserSaveArchive>;

    fn new(path: impl AsRef<Path>, ctx: Self::Context) -> Result<Self, TreeError> {
        let path = path.as_ref().to_string_lossy().into_owned();
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(
                &mut handle,
                ctx.archive,
                data,
                FS_OPEN_CREATE as u32 | FS_OPEN_WRITE as u32,
                0,
            );

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to open {:?}", path),
                )));
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to close {:?}", path),
                )));
            }
        }

        Ok(Self {
            path,
            ctx: ctx.clone(),
        })
    }

    fn pad(&mut self, length: u64) -> Result<(), TreeError> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_WRITE as u32, 0);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to open {:?}", self.path),
                )));
            }

            let res = FSFILE_SetSize(handle, length);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to set size of {:?} to {} bytes", self.path, length),
                )));
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to close {:?}", self.path),
                )));
            }
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), TreeError> {
        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_DeleteFile(self.ctx.archive, data);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to delete {:?}", self.path),
                )));
            }
        }

        Ok(())
    }

    fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    fn data(&self) -> Result<impl io::Read + io::Seek, TreeError> {
        let mut handle = 0;
        let mut buf = vec![0u8; 0];

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to open {:?}", self.path),
                )));
            }

            let mut size: u64 = 0;
            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to get size in bytes of {:?}", self.path),
                )));
            }

            buf.resize(size as usize, 0x00);

            let mut bytes_read: u32 = 0;
            let res = FSFILE_Read(
                handle,
                &mut bytes_read,
                0,
                buf.as_mut_ptr() as *mut _,
                size as u32,
            );

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to read contents of {:?}", self.path),
                )));
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to close {:?}", self.path),
                )));
            }
        }

        Ok(Cursor::new(buf))
    }

    fn length(&self) -> Result<u64, TreeError> {
        let mut handle = 0;
        let mut size: u64 = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to open {:?}", self.path),
                )));
            }

            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to get size in bytes of {:?}", self.path),
                )));
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to close {:?}", self.path),
                )));
            }
        }

        Ok(size)
    }

    fn write_chunk(&mut self, offset: u64, source: &mut impl io::Read) -> Result<(), TreeError> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_WRITE as u32, 0);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to open {:?}", self.path),
                )));
            }

            let mut buf = Vec::new();
            source.read_to_end(&mut buf)?;

            let mut bytes_written: u32 = 0;
            let res = FSFILE_Write(
                handle,
                &mut bytes_written,
                offset,
                buf.as_ptr() as *const _,
                buf.len() as u32,
                FS_WRITE_FLUSH as u32,
            );

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!(
                        "Failed to write source buffer to {:?} with {:x}",
                        self.path,
                        res
                    ),
                )));
            }

            if bytes_written != buf.len() as u32 {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!(
                        "Short write, {} written, {} expected",
                        bytes_written,
                        buf.len()
                    ),
                )));
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                return Err(TreeError::Io(io::Error::new(
                    io::ErrorKind::Other,
                    anyhow!("failed to close {:?}", self.path),
                )));
            }
        }

        Ok(())
    }
}

pub fn walk_tree(title_id: u64) -> Result<Tree<ArchiveFileLeaf>> {
    let ctx = Arc::new(CtruUserSaveArchive::open(title_id)?);
    let mut results = HashMap::new();

    walk_sub("/\0".encode_utf16().collect(), &ctx, &mut results);

    /// Note utf16 paths used here and converted when inserted to HashMap
    fn walk_sub(
        dir_path: Vec<u16>,
        ctx: &<ArchiveFileLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, ArchiveFileLeaf>,
    ) -> Result<()> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, dir_path.as_ptr() as *const _);

            let res = FSUSER_OpenDirectory(&mut handle, ctx.archive, data);

            if R_FAILED(res) {
                bail!(
                    "Failed to open directory {:?}",
                    String::from_utf16_lossy(&dir_path)
                );
            }

            let mut entries: Vec<FS_DirectoryEntry> = vec![std::mem::zeroed(); 16];
            let mut entries_read = 0;

            let res = FSDIR_Read(
                handle,
                &mut entries_read,
                entries.len() as u32,
                entries.as_mut_ptr(),
            );

            if R_FAILED(res) {
                bail!("failed to list directory {:?}", dir_path);
            }

            entries.truncate(entries_read as usize);

            for entry in entries {
                let null_term_pos = entry
                    .name
                    .iter()
                    .position(|&c| c == 0x00)
                    .expect("null terminator on FS_DirectoryEntry");

                let mut fq_path = dir_path[..dir_path.len() - 1]
                    .iter()
                    .chain(entry.name[..=null_term_pos].iter())
                    .copied()
                    .collect::<Vec<_>>();

                if entry.attributes & FS_ATTRIBUTE_DIRECTORY != 0 {
                    fq_path.push('/' as u16);
                    walk_sub(fq_path, ctx, results);
                } else {
                    let fq_path = PathBuf::from(String::from_utf16_lossy(&fq_path));
                    let leaf = ArchiveFileLeaf::new(&fq_path, ctx.clone()).expect("leaf created");
                    results.insert(fq_path, leaf);
                }
            }

            Ok(())
        }
    }

    Ok(Tree::new(results, ctx))
}
