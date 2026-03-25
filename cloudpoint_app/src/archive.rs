use anyhow::{Result, anyhow, bail};
use chunktree::tree::{Leaf, Tree, TreeError};
use cloudpoint_lib::sync::CtrArchiveMode;
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    AM_GetTitleExtDataId, ARCHIVE_ACTION_COMMIT_SAVE_DATA, FS_ATTRIBUTE_DIRECTORY, FS_Archive,
    FS_DirectoryEntry, FS_OPEN_READ, FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, FSDIR_Close,
    FSDIR_Read, FSFILE_Close, FSFILE_GetSize, FSFILE_Read, FSFILE_SetSize, FSFILE_Write,
    FSUSER_CloseArchive, FSUSER_ControlArchive, FSUSER_ControlSecureSave, FSUSER_CreateDirectory,
    FSUSER_CreateFile, FSUSER_DeleteFile, FSUSER_OpenArchive, FSUSER_OpenDirectory,
    FSUSER_OpenFile, PATH_ASCII, PATH_BINARY, R_FAILED, SECURESAVE_ACTION_DELETE,
    SECUREVALUE_SLOT_SD, fsExit, fsInit, fsMakePath,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CtrArchive {
    pub title_id: u64,
    pub mode: CtrArchiveMode,
    pub archive: FS_Archive,
}

impl CtrArchive {
    pub fn open(title_id: u64, mode: CtrArchiveMode) -> Result<Self> {
        let mut archive: FS_Archive = 0;

        unsafe {
            let res = fsInit();

            if R_FAILED(res) {
                fsExit();
                bail!("could not retrieve extdata identifier for {}", title_id);
            }

            let (path_data, archive_id) = match mode {
                CtrArchiveMode::Savedata => (
                    [
                        MediaType::Sd as u32,
                        title_id as u32,
                        (title_id >> 32) as u32,
                    ],
                    ArchiveID::UserSavedata as u32,
                ),
                CtrArchiveMode::Extdata => {
                    let mut extdata_id: u64 = 0;
                    let res = AM_GetTitleExtDataId(&mut extdata_id, MediaType::Sd as u8, title_id);

                    if R_FAILED(res) {}

                    (
                        [MediaType::Sd as u32, extdata_id as u32, 0],
                        ArchiveID::Extdata as u32,
                    )
                }
            };

            let path = FS_Path {
                type_: PATH_BINARY,
                size: 12,
                data: path_data.as_ptr() as *const c_void,
            };

            let res = FSUSER_OpenArchive(&mut archive, archive_id, path);

            if R_FAILED(res) {
                fsExit();
                bail!("could not open archive for {}", title_id);
            }
        }

        Ok(Self {
            title_id,
            mode,
            archive,
        })
    }
}

impl Drop for CtrArchive {
    fn drop(&mut self) {
        unsafe {
            if self.mode == CtrArchiveMode::Savedata {
                let res = FSUSER_ControlArchive(
                    self.archive,
                    ARCHIVE_ACTION_COMMIT_SAVE_DATA,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    0,
                );

                if R_FAILED(res) {
                    fsExit();
                    panic!(
                        "failed to commit archive {} for title {}",
                        self.archive, self.title_id
                    );
                }

                let mut out: u8 = 0;
                let low_id = self.title_id as u32;
                let mut secure_value: u64 =
                    ((SECUREVALUE_SLOT_SD as u64) << 32) | ((low_id & 0xFFFFFF00) as u64);
                let res = FSUSER_ControlSecureSave(
                    SECURESAVE_ACTION_DELETE,
                    &mut secure_value as *mut u64 as *mut _,
                    8,
                    &mut out as *mut u8 as *mut _,
                    1,
                );

                if R_FAILED(res) {
                    fsExit();
                    panic!(
                        "failed to reset secure save metadata for title {}",
                        self.title_id
                    );
                }
            }

            let res = FSUSER_CloseArchive(self.archive);

            if R_FAILED(res) {
                fsExit();
                panic!("failed to close archive for {}", self.title_id)
            }

            fsExit();
        }
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
        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);
            let mut handle = 0;

            match self.ctx.mode {
                CtrArchiveMode::Savedata => {
                    let res = FSUSER_OpenFile(
                        &mut handle,
                        self.ctx.archive,
                        data,
                        FS_OPEN_WRITE as u32,
                        0,
                    );

                    match res as u32 {
                        // Opened, resize
                        0 => {
                            let mut curr_length = 0;
                            let res = FSFILE_GetSize(handle, &mut curr_length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to get current size of {:?})", self.path,),
                                )));
                            }

                            if curr_length == length {
                                let res = FSFILE_Close(handle);

                                if R_FAILED(res) {
                                    return Err(TreeError::Io(io::Error::new(
                                        io::ErrorKind::Other,
                                        anyhow!("failed to close {:?} during resize", self.path),
                                    )));
                                }

                                return Ok(());
                            }

                            let res = FSFILE_SetSize(handle, length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!(
                                        "failed to set size of {:?} to {} bytes",
                                        self.path,
                                        length
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
                        // Not found, create
                        0xc8804470 | 0xc8804471 => {
                            let slash_pos = self
                                .path
                                .char_indices()
                                .filter_map(|(i, c)| (c == '/').then_some(i))
                                .skip(1)
                                .collect::<Vec<_>>();

                            for i in slash_pos {
                                let p = format!("{}\0", &self.path[0..=i]);
                                let dir_data = fsMakePath(PATH_ASCII, p.as_ptr() as *const _);
                                let res =
                                    FSUSER_OpenDirectory(&mut handle, self.ctx.archive, dir_data);

                                if R_FAILED(res) {
                                    let res = FSUSER_CreateDirectory(self.ctx.archive, dir_data, 0);

                                    if R_FAILED(res) {
                                        return Err(TreeError::Io(io::Error::new(
                                            io::ErrorKind::Other,
                                            anyhow!(
                                                "failed to create intermediary dir {} ({:0x})",
                                                p,
                                                res
                                            )
                                            .context("pad"),
                                        )));
                                    }
                                } else {
                                    let res = FSDIR_Close(handle);

                                    if R_FAILED(res) {
                                        return Err(TreeError::Io(io::Error::new(
                                            io::ErrorKind::Other,
                                            anyhow!(
                                                "failed to close intermediary dir {p} during resize ({:016x})",
                                                res
                                            ),
                                        )));
                                    }
                                }
                            }

                            let res = FSUSER_CreateFile(self.ctx.archive, data, 0, length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to create {:?}, {:0x}", self.path, res)
                                        .context("pad"),
                                )));
                            }
                        }
                        // Other failure
                        _ => {
                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to open {:?}", self.path).context("pad save"),
                                )));
                            }
                        }
                    }
                }
                CtrArchiveMode::Extdata => {
                    let res = FSUSER_OpenFile(
                        &mut handle,
                        self.ctx.archive,
                        data,
                        FS_OPEN_READ as u32,
                        0,
                    );

                    match res as u32 {
                        // Opened, resize via recreate (extdata limitation)
                        0 => {
                            let mut curr_length = 0;
                            let res = FSFILE_GetSize(handle, &mut curr_length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to get current size of {:?})", self.path,),
                                )));
                            }

                            let res = FSFILE_Close(handle);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to close {:?} during resize", self.path),
                                )));
                            }

                            if curr_length == length {
                                return Ok(());
                            }

                            let mut curr_data = vec![0x00; curr_length as usize];
                            self.data()?.read_exact(&mut curr_data)?;
                            curr_data.resize(length as usize, 0x00);

                            self.delete()?;

                            let res = FSUSER_CreateFile(self.ctx.archive, data, 0, length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to create {:?} during resize", self.path),
                                )));
                            }

                            self.write_chunk(0, &mut Cursor::new(curr_data))?;
                        }
                        // Not found, create
                        0xc8804470 | 0xc8804471 => {
                            let slash_pos = self
                                .path
                                .char_indices()
                                .filter_map(|(i, c)| (c == '/').then_some(i))
                                .skip(1)
                                .collect::<Vec<_>>();

                            for i in slash_pos {
                                let p = format!("{}\0", &self.path[0..=i]);
                                let dir_data = fsMakePath(PATH_ASCII, p.as_ptr() as *const _);
                                let res =
                                    FSUSER_OpenDirectory(&mut handle, self.ctx.archive, dir_data);

                                if R_FAILED(res) {
                                    let res = FSUSER_CreateDirectory(self.ctx.archive, dir_data, 0);

                                    if R_FAILED(res) {
                                        return Err(TreeError::Io(io::Error::new(
                                            io::ErrorKind::Other,
                                            anyhow!(
                                                "failed to create intermediary dir {} ({:0x})",
                                                p,
                                                res
                                            )
                                            .context("pad"),
                                        )));
                                    }
                                } else {
                                    let res = FSDIR_Close(handle);

                                    if R_FAILED(res) {
                                        return Err(TreeError::Io(io::Error::new(
                                            io::ErrorKind::Other,
                                            anyhow!(
                                                "failed to close intermediary dir {p} during resize ({:016x})",
                                                res
                                            ),
                                        )));
                                    }
                                }
                            }

                            let res = FSUSER_CreateFile(self.ctx.archive, data, 0, length);

                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to create {:?}, {:0x}", self.path, res)
                                        .context("pad"),
                                )));
                            }
                        }
                        // Other failure
                        _ => {
                            if R_FAILED(res) {
                                return Err(TreeError::Io(io::Error::new(
                                    io::ErrorKind::Other,
                                    anyhow!("failed to open {:?} ({:0x})", self.path, res)
                                        .context("pad extdata"),
                                )));
                            }
                        }
                    }
                }
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
                    anyhow!("failed to open {:?}", self.path).context("data"),
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
                    anyhow!("failed to open {:?}", self.path).context("length"),
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
                    anyhow!("failed to open {:?}", self.path).context("write chunk"),
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

pub fn walk_tree(title_id: u64, mode: CtrArchiveMode) -> Result<Tree<CtrArchiveLeaf>> {
    let ctx = Arc::new(CtrArchive::open(title_id, mode)?);
    let mut results = HashMap::new();

    walk_sub("/".into(), &ctx, &mut results)?;

    fn walk_sub(
        dir_path: String,
        ctx: &<CtrArchiveLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, CtrArchiveLeaf>,
    ) -> Result<()> {
        let mut handle = 0;

        unsafe {
            let with_null = format!("{dir_path}\0");
            let data = fsMakePath(PATH_ASCII, with_null.as_ptr() as *const _);
            let res = FSUSER_OpenDirectory(&mut handle, ctx.archive, data);

            if R_FAILED(res) {
                bail!("Failed to open directory {:?}", (&dir_path));
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
    }

    Ok(Tree::new(results, ctx))
}
