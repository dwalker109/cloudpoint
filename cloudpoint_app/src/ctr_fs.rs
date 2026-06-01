use anyhow::Result;
use cloudpoint_lib::{ctr::CtrSmdh, sync::SyncItem};
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    FS_DirectoryEntry, FS_OPEN_READ, FS_Path, Handle, PATH_ASCII, PATH_BINARY, PATH_EMPTY,
    fsMakePath,
};
use ffi::{
    ctr_close_archive, ctr_close_directory, ctr_close_file, ctr_commit_archive,
    ctr_create_directory, ctr_create_file, ctr_delete_file, ctr_get_file_size, ctr_open_archive,
    ctr_open_directory, ctr_open_file, ctr_read_directory, ctr_read_ext_smdh, ctr_read_file,
    ctr_read_title_smdh, ctr_reset_secure_save_meta, ctr_set_file_size, ctr_write_file,
};
use std::ffi::{CString, c_void};
use std::io::{self, Error as IoError, Read, Seek, SeekFrom};

mod ffi;

pub struct CtrNand {
    nand_handle: u64,
}

impl CtrNand {
    pub fn open() -> Result<Self, IoError> {
        let path = unsafe { fsMakePath(PATH_EMPTY, b"\0".as_ptr() as _) };
        let nand_handle = ctr_open_archive(ArchiveID::NandCtrFS, path)?;

        Ok(Self { nand_handle })
    }

    pub fn movable_sed(&self) -> Result<Vec<u8>, IoError> {
        let path = CtrFsPath::new("/private/movable.sed")?;
        let file_handle = ctr_open_file(self.nand_handle, path.fs_path(), FS_OPEN_READ)?;
        let size = ctr_get_file_size(file_handle)?;
        let mut file = CtrFile {
            file_handle,
            size,
            pos: 0,
        };
        let data = file.read_to_vec(0, 288)?;

        Ok(data)
    }
}

struct CtrArchivePath {
    _sync_item: SyncItem,
    buffer: [u32; 3],
    archive_id: ArchiveID,
}

impl CtrArchivePath {
    fn new(sync_item: SyncItem) -> Result<Self, IoError> {
        let (buffer, archive_id) = match sync_item {
            SyncItem::Savedata(title_id) => (
                [
                    MediaType::Sd as u32,
                    title_id as u32,
                    (title_id >> 32) as u32,
                ],
                ArchiveID::UserSavedata,
            ),
            SyncItem::Extdata(extdata_id) => (
                [MediaType::Sd as u32, extdata_id as u32, 0],
                ArchiveID::Extdata,
            ),
        };

        Ok(Self {
            _sync_item: sync_item,
            buffer,
            archive_id,
        })
    }

    fn fs_path(&self) -> FS_Path {
        FS_Path {
            type_: PATH_BINARY,
            size: 12,
            data: self.buffer.as_ptr() as *const c_void,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CtrArchive {
    sync_item: SyncItem,
    archive_handle: u64,
}

impl CtrArchive {
    pub fn smdh(sync_item: SyncItem) -> Result<CtrSmdh, IoError> {
        log::debug!("fetching smdh for {}", sync_item);

        match sync_item {
            SyncItem::Savedata(title_id) => Ok(ctr_read_title_smdh(title_id)?.into()),
            SyncItem::Extdata(extdata_id) => Ok(ctr_read_ext_smdh(extdata_id)?.into()),
        }
    }

    pub fn open(sync_item: SyncItem) -> Result<Self, IoError> {
        log::debug!("opening archive for {}", sync_item);

        let path = CtrArchivePath::new(sync_item)?;
        let handle = ctr_open_archive(path.archive_id, path.fs_path())?;

        Ok(Self {
            sync_item,
            archive_handle: handle,
        })
    }

    pub fn sync_item(&self) -> &SyncItem {
        &self.sync_item
    }

    pub fn open_file(&self, path: &CtrFsPath, flags: u8) -> Result<CtrFile, IoError> {
        log::debug!("opening file {:?} in archive for {}", path, self.sync_item);

        let file_handle = ctr_open_file(self.archive_handle, path.fs_path(), flags)?;
        let size = ctr_get_file_size(file_handle)?;

        Ok(CtrFile {
            file_handle,
            size,
            pos: 0,
        })
    }

    pub fn create_file(&self, path: &CtrFsPath, size: u64) -> Result<(), IoError> {
        log::debug!("creating file {:?} in archive for {}", path, self.sync_item);

        ctr_create_file(self.archive_handle, path.fs_path(), size)
    }

    pub fn delete_file(&self, path: &CtrFsPath) -> Result<(), IoError> {
        log::debug!("deleting file {:?} in archive for {}", path, self.sync_item);

        ctr_delete_file(self.archive_handle, path.fs_path())
    }

    pub fn open_directory(&self, path: &CtrFsPath) -> Result<CtrDirectory, IoError> {
        log::debug!(
            "opening directory {:?} in archive for {}",
            path,
            self.sync_item
        );

        Ok(CtrDirectory {
            directory_handle: ctr_open_directory(self.archive_handle, path.fs_path())?,
        })
    }

    pub fn create_directory(&self, path: &CtrFsPath) -> Result<(), IoError> {
        log::debug!(
            "creating directory {:?} in archive for {}",
            path,
            self.sync_item
        );

        ctr_create_directory(self.archive_handle, path.fs_path())
    }

    pub fn finalise(&self) -> Result<(), IoError> {
        log::debug!("finalising save write in archive for {}", self.sync_item);

        if let SyncItem::Savedata(title_id) = self.sync_item {
            ctr_commit_archive(self.archive_handle)?;
            ctr_reset_secure_save_meta(title_id)?;
        }

        Ok(())
    }
}

impl Drop for CtrArchive {
    fn drop(&mut self) {
        log::debug!("dropping archive for {}", self.sync_item);
        ctr_close_archive(self.archive_handle).expect("archive should be closable");
    }
}

#[derive(Debug)]
pub struct CtrFsPath(CString);

impl CtrFsPath {
    pub fn new(path: &str) -> Result<Self, IoError> {
        Ok(Self(CString::new(path)?))
    }

    pub fn fs_path(&self) -> FS_Path {
        unsafe { fsMakePath(PATH_ASCII, self.0.as_ptr() as *const _) }
    }
}

pub struct CtrFile {
    file_handle: Handle,
    pos: u64,
    size: u64,
}

impl CtrFile {
    pub fn read_to_vec(&mut self, offset: u64, length: u64) -> Result<Vec<u8>, IoError> {
        log::debug!(
            "reading to owned vec from handle {} at offset {} with length {}",
            self.file_handle,
            offset,
            length
        );

        let mut buf = vec![0u8; length as usize];
        self.read_exact(&mut buf)?;

        Ok(buf)
    }

    pub fn write(&self, offset: u64, buffer: &[u8], flags: u16) -> Result<(), IoError> {
        log::debug!(
            "writing to handle {} at offset {} with length {}",
            self.file_handle,
            offset,
            buffer.len()
        );

        ctr_write_file(self.file_handle, offset, buffer, flags)
    }

    pub fn size(&self) -> Result<u64, IoError> {
        log::debug!("getting size of file at handle {}", self.file_handle,);

        ctr_get_file_size(self.file_handle)
    }

    pub fn set_size(&self, size: u64) -> Result<(), IoError> {
        log::debug!(
            "setting size of file at handle {} to {}",
            self.file_handle,
            size
        );

        ctr_set_file_size(self.file_handle, size)
    }
}

impl Read for CtrFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        log::debug!(
            "reading to provided buffer (of len {}) from handle {} at offset {}",
            buf.len(),
            self.file_handle,
            self.pos,
        );

        let bytes_to_eof = match self.size.checked_sub(self.pos) {
            Some(0) | None => {
                return Ok(0);
            }
            Some(n) => n,
        };

        let bytes_to_read = (buf.len() as u64).min(bytes_to_eof) as usize;

        let n = ctr_read_file(self.file_handle, self.pos, &mut buf[..bytes_to_read])?;
        self.pos += n;

        Ok(n as usize)
    }
}

impl Seek for CtrFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => self.size as i64 + n,
            SeekFrom::Current(n) => self.pos as i64 + n,
        };

        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot seek to before start",
            ));
        }

        self.pos = new_pos as u64;

        Ok(self.pos)
    }
}

impl Drop for CtrFile {
    fn drop(&mut self) {
        log::debug!("dropping handle {}", self.file_handle);
        ctr_close_file(self.file_handle).expect("file should be closable");
    }
}

pub struct CtrDirectory {
    directory_handle: Handle,
}

impl CtrDirectory {
    pub fn read(&self) -> Result<Vec<FS_DirectoryEntry>, IoError> {
        log::debug!("reading directory at handle {}", self.directory_handle,);

        ctr_read_directory(self.directory_handle)
    }
}

impl Drop for CtrDirectory {
    fn drop(&mut self) {
        log::debug!("dropping handle {}", self.directory_handle);
        ctr_close_directory(self.directory_handle).expect("dir should be closable");
    }
}
