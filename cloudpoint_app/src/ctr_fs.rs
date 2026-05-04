use anyhow::Result;
use cloudpoint_lib::ctr::{CtrArchiveId, CtrSmdh};
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{FS_DirectoryEntry, FS_Path, Handle, PATH_ASCII, PATH_BINARY, fsMakePath};
use ffi::{
    ctr_close_archive, ctr_close_directory, ctr_close_file, ctr_commit_archive,
    ctr_create_directory, ctr_create_file, ctr_delete_file, ctr_get_file_size, ctr_open_archive,
    ctr_open_directory, ctr_open_file, ctr_read_directory, ctr_read_ext_smdh, ctr_read_file,
    ctr_read_title_smdh, ctr_reset_secure_save_meta, ctr_set_file_size, ctr_write_file,
};
use std::ffi::{CString, c_void};
use std::io::Error as IoError;

mod ffi;

struct CtrArchivePath {
    _ctr_archive_id: CtrArchiveId,
    buffer: [u32; 3],
    archive_id: ArchiveID,
}

impl CtrArchivePath {
    fn new(ctr_archive_id: CtrArchiveId) -> Result<Self, IoError> {
        let (buffer, archive_id) = match ctr_archive_id {
            CtrArchiveId::Savedata(title_id) => (
                [
                    MediaType::Sd as u32,
                    title_id as u32,
                    (title_id >> 32) as u32,
                ],
                ArchiveID::UserSavedata,
            ),
            CtrArchiveId::Extdata(extdata_id) => (
                [MediaType::Sd as u32, extdata_id as u32, 0],
                ArchiveID::Extdata,
            ),
        };

        Ok(Self {
            _ctr_archive_id: ctr_archive_id,
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
    archive_id: CtrArchiveId,
    archive_handle: u64,
}

impl CtrArchive {
    pub fn smdh(archive_id: CtrArchiveId) -> Result<CtrSmdh, IoError> {
        match archive_id {
            CtrArchiveId::Savedata(title_id) => Ok(ctr_read_title_smdh(title_id)?.into()),
            CtrArchiveId::Extdata(extdata_id) => Ok(ctr_read_ext_smdh(extdata_id)?.into()),
        }
    }

    pub fn open(archive_id: CtrArchiveId) -> Result<Self, IoError> {
        let path = CtrArchivePath::new(archive_id)?;
        let handle = ctr_open_archive(path.archive_id, path.fs_path())?;

        Ok(Self {
            archive_id,
            archive_handle: handle,
        })
    }

    pub fn archive_id(&self) -> &CtrArchiveId {
        &self.archive_id
    }

    pub fn open_file(&self, path: &CtrFsPath, flags: u8) -> Result<CtrFile, IoError> {
        Ok(CtrFile {
            file_handle: ctr_open_file(self.archive_handle, path.fs_path(), flags)?,
        })
    }

    pub fn create_file(&self, path: &CtrFsPath, size: u64) -> Result<(), IoError> {
        ctr_create_file(self.archive_handle, path.fs_path(), size)
    }

    pub fn delete_file(&self, path: &CtrFsPath) -> Result<(), IoError> {
        ctr_delete_file(self.archive_handle, path.fs_path())
    }

    pub fn open_directory(&self, path: &CtrFsPath) -> Result<CtrDirectory, IoError> {
        Ok(CtrDirectory {
            directory_handle: ctr_open_directory(self.archive_handle, path.fs_path())?,
        })
    }

    pub fn create_directory(&self, path: &CtrFsPath) -> Result<(), IoError> {
        ctr_create_directory(self.archive_handle, path.fs_path())
    }

    pub fn finalise(&self) -> Result<(), IoError> {
        if let CtrArchiveId::Savedata(title_id) = self.archive_id {
            ctr_commit_archive(self.archive_handle)?;
            ctr_reset_secure_save_meta(title_id)?;
        }

        Ok(())
    }
}

impl Drop for CtrArchive {
    fn drop(&mut self) {
        ctr_close_archive(self.archive_handle).expect("archive should be closable");
    }
}

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
}

impl CtrFile {
    pub fn read(&self, offset: u64, length: u64) -> Result<Vec<u8>, IoError> {
        ctr_read_file(self.file_handle, offset, length)
    }

    pub fn write(&self, offset: u64, buffer: &[u8], flags: u16) -> Result<(), IoError> {
        ctr_write_file(self.file_handle, offset, buffer, flags)
    }

    pub fn size(&self) -> Result<u64, IoError> {
        ctr_get_file_size(self.file_handle)
    }

    pub fn set_size(&self, size: u64) -> Result<(), IoError> {
        ctr_set_file_size(self.file_handle, size)
    }
}

impl Drop for CtrFile {
    fn drop(&mut self) {
        ctr_close_file(self.file_handle).expect("file should be closable");
    }
}

pub struct CtrDirectory {
    directory_handle: Handle,
}

impl CtrDirectory {
    pub fn read(&self) -> Result<Vec<FS_DirectoryEntry>, IoError> {
        ctr_read_directory(self.directory_handle)
    }
}

impl Drop for CtrDirectory {
    fn drop(&mut self) {
        ctr_close_directory(self.directory_handle).expect("dir should be closable");
    }
}
