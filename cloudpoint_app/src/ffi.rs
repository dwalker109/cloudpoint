use crate::ctr_archive::CtrArchive;
use anyhow::anyhow;
use cloudpoint_lib::sync::CtrArchiveKind;
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    AM_GetTitleExtDataId, ARCHIVE_ACTION_COMMIT_SAVE_DATA, FS_Archive, FS_DirectoryEntry, FS_Path,
    FSDIR_Close, FSDIR_Read, FSFILE_Close, FSFILE_GetSize, FSFILE_Read, FSFILE_SetSize,
    FSFILE_Write, FSUSER_CloseArchive, FSUSER_ControlArchive, FSUSER_ControlSecureSave,
    FSUSER_CreateDirectory, FSUSER_CreateFile, FSUSER_DeleteFile, FSUSER_OpenArchive,
    FSUSER_OpenDirectory, FSUSER_OpenFile, Handle, PATH_ASCII, PATH_BINARY, R_FAILED,
    SECURESAVE_ACTION_DELETE, SECUREVALUE_SLOT_SD, fsMakePath,
};
use std::ffi::c_void;
use std::io::{Error as IoError, ErrorKind as IoErrorKind};

pub use ctru_sys::{FS_ATTRIBUTE_DIRECTORY, FS_OPEN_READ, FS_OPEN_WRITE, FS_WRITE_FLUSH};

pub struct CtrArchivePath {
    pub title_id: u64,
    buffer: [u32; 3],
    pub archive_id: ArchiveID,
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
                let mut extdata_id: u64 = 0;

                let res =
                    unsafe { AM_GetTitleExtDataId(&mut extdata_id, MediaType::Sd as u8, title_id) };

                if R_FAILED(res) {
                    return Err(IoError::new(
                        IoErrorKind::Other,
                        anyhow!(
                            "could not retrieve extdata_id for title {:016X} [{:#010X}]",
                            title_id,
                            res
                        ),
                    ));
                }

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

pub struct CtrFilePath(String);

impl CtrFilePath {
    pub fn new(path: &str) -> Self {
        Self(path.into())
    }

    pub fn fs_path(&self) -> FS_Path {
        unsafe { fsMakePath(PATH_ASCII, self.0.as_ptr() as *const _) }
    }
}

pub fn ctr_open_archive(path: &CtrArchivePath) -> Result<FS_Archive, IoError> {
    let mut archive_handle: FS_Archive = 0;

    let res =
        unsafe { FSUSER_OpenArchive(&mut archive_handle, path.archive_id as u32, path.fs_path()) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open archive for title {:016X} [{:#010X}]",
                path.title_id,
                res
            ),
        ));
    }

    Ok(archive_handle)
}

pub fn ctr_close_archive(archive: &CtrArchive) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CloseArchive(archive.handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close archive for title {:016X} [{:#010X}]",
                archive.title_id,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_open_directory(archive: &CtrArchive, path: &CtrFilePath) -> Result<Handle, IoError> {
    let mut handle: Handle = 0;

    let res = unsafe { FSUSER_OpenDirectory(&mut handle, archive.handle, path.fs_path()) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open directory at path \"{}\" for title {:016X} [{:#010X}]",
                path.0,
                archive.title_id,
                res
            ),
        ));
    }

    Ok(handle)
}

pub fn ctr_read_directory(handle: Handle) -> Result<Vec<FS_DirectoryEntry>, IoError> {
    let mut entries: Vec<FS_DirectoryEntry> = vec![unsafe { std::mem::zeroed() }; 256];
    let mut entries_read = 0;

    let res = unsafe {
        FSDIR_Read(
            handle,
            &mut entries_read,
            entries.len() as u32,
            entries.as_mut_ptr(),
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not read directory handle \"{}\" [{:#010X}]",
                handle,
                res
            ),
        ));
    }

    entries.truncate(entries_read as usize);

    Ok(entries)
}

pub fn ctr_create_directory(archive: &CtrArchive, path: &CtrFilePath) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CreateDirectory(archive.handle, path.fs_path(), 0) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not create directory at path \"{}\" for title {:016X} [{:#010X}]",
                path.0,
                archive.title_id,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_close_directory(handle: Handle) -> Result<(), IoError> {
    let res = unsafe { FSDIR_Close(handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!("could not close directory handle {} [{:010X}]", handle, res),
        ));
    }

    Ok(())
}

pub fn ctr_open_file(
    archive: &CtrArchive,
    path: &CtrFilePath,
    flags: u8,
) -> Result<Handle, IoError> {
    let mut handle: Handle = 0;

    let res =
        unsafe { FSUSER_OpenFile(&mut handle, archive.handle, path.fs_path(), flags as u32, 0) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open file at path \"{}\" for title {:016X} [{:#010X}]",
                path.0,
                archive.title_id,
                res
            ),
        ));
    }

    Ok(handle)
}

pub fn ctr_read_file(handle: Handle, offset: u64, length: u64) -> Result<Vec<u8>, IoError> {
    let mut buffer = vec![0u8; length as usize];
    let mut bytes_read: u32 = 0;

    let res = unsafe {
        FSFILE_Read(
            handle,
            &mut bytes_read,
            offset,
            buffer.as_mut_ptr() as *mut _,
            length as u32,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not read bytes {} to {} of handle {} [{:#010X}]",
                offset,
                offset + length,
                handle,
                res
            ),
        ));
    }

    if bytes_read != length as u32 {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "wrong amount of bytes were read ({}/{}) of handle {} [{:#010X}]",
                length,
                bytes_read,
                handle,
                res
            ),
        ));
    }

    Ok(buffer)
}

pub fn ctr_write_file(
    handle: Handle,
    offset: u64,
    buffer: &[u8],
    flags: u16,
) -> Result<(), IoError> {
    let mut bytes_written: u32 = 0;

    let res = unsafe {
        FSFILE_Write(
            handle,
            &mut bytes_written,
            offset,
            buffer.as_ptr() as *const _,
            buffer.len() as u32,
            flags as u32,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not write source buffer to handle {} [{:#010X}]",
                handle,
                res
            ),
        ));
    }

    if bytes_written != buffer.len() as u32 {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "wrong amount of bytes were written ({}/{}) to handle {} [{:#010X}]",
                buffer.len(),
                bytes_written,
                handle,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_create_file(archive: &CtrArchive, path: &CtrFilePath, size: u64) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CreateFile(archive.handle, path.fs_path(), 0, size) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not create file at path \"{}\" for title {:016X} [{:#010X}]",
                path.0,
                archive.title_id,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_close_file(handle: Handle) -> Result<(), IoError> {
    let res = unsafe { FSFILE_Close(handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!("could not close file handle {:?} [{:010X}]", handle, res),
        ));
    }

    Ok(())
}

pub fn ctr_delete_file(archive: &CtrArchive, path: &CtrFilePath) -> Result<(), IoError> {
    let res = unsafe { FSUSER_DeleteFile(archive.handle, path.fs_path()) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!("could not delete file at path {} [{:010X}]", path.0, res),
        ));
    }

    Ok(())
}

pub fn ctr_get_file_size(handle: Handle) -> Result<u64, IoError> {
    let mut output = 0;
    let res = unsafe { FSFILE_GetSize(handle, &mut output) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not get size of file at handle {:?} [{:010X}]",
                handle,
                res
            ),
        ));
    }

    Ok(output)
}

pub fn ctr_set_file_size(handle: Handle, size: u64) -> Result<(), IoError> {
    let res = unsafe { FSFILE_SetSize(handle, size) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not set size of file at handle {:?} to {} bytes [{:010X}]",
                handle,
                size,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_commit_archive(archive: &CtrArchive) -> Result<(), IoError> {
    if archive.kind != CtrArchiveKind::Savedata {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!("archive commit is only supported for Savedata",),
        ));
    }

    let res = unsafe {
        FSUSER_ControlArchive(
            archive.handle,
            ARCHIVE_ACTION_COMMIT_SAVE_DATA,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not commit archive for title {:016X} [{:#010X}]",
                archive.title_id,
                res
            ),
        ));
    }

    Ok(())
}

pub fn ctr_reset_secure_save_meta(title_id: u64) -> Result<(), IoError> {
    let mut input: u64 =
        ((SECUREVALUE_SLOT_SD as u64) << 32) | ((title_id as u32 & 0xffffff00) as u64);
    let mut output: u8 = 0;

    let res = unsafe {
        FSUSER_ControlSecureSave(
            SECURESAVE_ACTION_DELETE,
            &mut input as *mut u64 as *mut _,
            8,
            &mut output as *mut u8 as *mut _,
            1,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "failed to reset secure save meta for title {:016X} [{:#010X}]",
                title_id,
                res
            ),
        ));
    }

    Ok(())
}
