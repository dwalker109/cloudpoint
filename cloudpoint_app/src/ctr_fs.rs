use anyhow::Result;
use cloudpoint_lib::sync::CtrArchiveKind;
use cloudpoint_lib::version::CtrMeta;
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{FS_DirectoryEntry, FS_Path, Handle, PATH_ASCII, PATH_BINARY, fsMakePath};
use ffi::{
    ctr_close_archive, ctr_close_directory, ctr_close_file, ctr_commit_archive,
    ctr_create_directory, ctr_create_file, ctr_delete_file, ctr_get_file_size,
    ctr_getr_ext_data_id_for_title, ctr_open_archive, ctr_open_directory, ctr_open_file,
    ctr_read_directory, ctr_read_file, ctr_reset_secure_save_meta, ctr_set_file_size,
    ctr_write_file,
};
use std::ffi::{CString, c_void};
use std::io::Error as IoError;

use crate::ctr_fs::ffi::{
    ctr_create_ext_save_data, ctr_format_savedata, ctr_get_format_info, ctr_get_title_version,
};

mod ffi;

pub struct CtrArchivePath {
    _title_id: u64,
    buffer: [u32; 3],
    archive_id: ArchiveID,
}

impl CtrArchivePath {
    pub fn new(title_id: u64, kind: CtrArchiveKind) -> Result<Self, IoError> {
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
            _title_id: title_id,
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

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CtrArchive {
    title_id: u64,
    kind: CtrArchiveKind,
    archive_handle: u64,
}

impl CtrArchive {
    pub fn meta(title_id: u64, kind: CtrArchiveKind) -> Result<CtrMeta, IoError> {
        let Ok(title_version) = ctr_get_title_version(title_id) else {
            return Ok(CtrMeta::Unavailable);
        };

        let path = CtrArchivePath::new(title_id, kind)?;

        let Ok((total_size, num_directories, num_files, duplicate_data)) =
            ctr_get_format_info(path.archive_id, path.fs_path())
        else {
            return Ok(CtrMeta::NotInitialized { title_version });
        };

        Ok(CtrMeta::Initialized {
            title_version,
            total_size,
            num_directories,
            num_files,
            duplicate_data,
        })
    }

    pub fn format_new(title_id: u64, kind: CtrArchiveKind, meta: CtrMeta) -> Result<(), IoError> {
        let (size, directories, files, duplicate_data) = meta
            .format_options()
            .expect("format options should be provided");

        let path = CtrArchivePath::new(title_id, kind)?;

        match kind {
            CtrArchiveKind::Savedata => {
                ctr_format_savedata(path.fs_path(), size, directories, files, duplicate_data)?;
            }
            CtrArchiveKind::Extdata => {
                let extdata_id = ctr_getr_ext_data_id_for_title(title_id)?;

                ctr_create_ext_save_data(extdata_id, directories, files)?;
            }
        }

        Ok(())
    }

    pub fn open(title_id: u64, kind: CtrArchiveKind) -> Result<Self, IoError> {
        let path = CtrArchivePath::new(title_id, kind)?;
        let handle = ctr_open_archive(path.archive_id, path.fs_path())?;

        Ok(Self {
            title_id,
            kind,
            archive_handle: handle,
        })
    }

    pub fn kind(&self) -> CtrArchiveKind {
        self.kind
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
        if self.kind == CtrArchiveKind::Savedata {
            ctr_commit_archive(self.archive_handle)?;
            ctr_reset_secure_save_meta(self.title_id)?;
        }

        Ok(())
    }
}

fn ctr_format_extdata(
    extdata_id: u64,
    size: u32,
    directories: u32,
    files: u32,
    duplicate_data: bool,
) -> std::result::Result<(), IoError> {
    todo!()
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
