use anyhow::anyhow;
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    AM_GetTitleExtDataId, AM_GetTitleInfo, AM_TitleInfo, ARCHIVE_ACTION_COMMIT_SAVE_DATA,
    FS_Archive, FS_DirectoryEntry, FS_ExtSaveDataInfo, FS_OPEN_READ, FS_Path, FSDIR_Close,
    FSDIR_Read, FSFILE_Close, FSFILE_GetSize, FSFILE_Read, FSFILE_SetSize, FSFILE_Write,
    FSUSER_CloseArchive, FSUSER_ControlArchive, FSUSER_ControlSecureSave, FSUSER_CreateDirectory,
    FSUSER_CreateFile, FSUSER_DeleteFile, FSUSER_OpenArchive, FSUSER_OpenDirectory,
    FSUSER_OpenFile, FSUSER_OpenFileDirectly, FSUSER_ReadExtSaveDataIcon, Handle, MEDIATYPE_SD,
    PATH_BINARY, R_FAILED, SECURESAVE_ACTION_DELETE, SECUREVALUE_SLOT_SD,
};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};

pub(super) fn ctr_get_title_version(title_id: u64) -> Result<u16, IoError> {
    let mut title_info: AM_TitleInfo = unsafe { std::mem::zeroed() };

    let res = unsafe {
        AM_GetTitleInfo(
            MEDIATYPE_SD,
            1,
            &title_id as *const u64 as _,
            &mut title_info,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not get title info for title {} [{:#010X}]",
                title_id,
                res
            ),
        ));
    }

    Ok(title_info.version)
}

pub(super) fn ctr_read_title_smdh(title_id: u64) -> Result<Vec<u8>, IoError> {
    let archive_path_data = [
        title_id as u32,
        (title_id >> 32) as u32,
        MediaType::Sd as u32,
        0x00000000u32,
    ];

    let archive_path = FS_Path {
        type_: PATH_BINARY,
        size: 16u32,
        data: archive_path_data.as_ptr() as *const _,
    };

    let file_path_data = [
        0x00000000u32,
        0x00000000,
        0x00000002,
        0x6E6F6369,
        0x00000000,
    ];

    let file_path = FS_Path {
        type_: PATH_BINARY,
        size: 20u32,
        data: file_path_data.as_ptr() as *const _,
    };

    let mut file_handle: Handle = 0;

    let res = unsafe {
        FSUSER_OpenFileDirectly(
            &mut file_handle,
            ArchiveID::SaveDataAndContent as u32,
            archive_path,
            file_path,
            FS_OPEN_READ as u32,
            0u32,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open smdh icon file for title {} [{:#010X}]",
                title_id,
                res
            ),
        ));
    }

    let smdh = ctr_read_file(file_handle, 0, 0x36c0)?;

    ctr_close_file(file_handle)?;

    Ok(smdh)
}

pub(super) fn ctr_read_ext_smdh(save_id: u64) -> Result<Vec<u8>, IoError> {
    let mut bytes_read = 0;

    let mut info: FS_ExtSaveDataInfo = unsafe { std::mem::zeroed() };
    info.set_mediaType(MEDIATYPE_SD as u8);
    info.saveId = save_id;

    let mut smdh = [0u8; 0x36c0];

    let res = unsafe {
        FSUSER_ReadExtSaveDataIcon(&mut bytes_read, info, 0x36c0, &mut smdh as *mut u8 as _)
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not read extdata smdh for id {} [{:#010X}]",
                save_id,
                res
            ),
        ));
    }

    Ok(smdh.to_vec())
}

pub(super) fn ctr_open_archive(
    archive_id: ArchiveID,
    path: FS_Path,
) -> Result<FS_Archive, IoError> {
    let mut archive_handle: FS_Archive = 0;

    let res = unsafe { FSUSER_OpenArchive(&mut archive_handle, archive_id as u32, path) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open archive of kind {:?} at path {:?} [{:#010X}]",
                archive_id,
                path,
                res
            ),
        ));
    }

    Ok(archive_handle)
}

pub(super) fn ctr_close_archive(archive_handle: FS_Archive) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CloseArchive(archive_handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close archive via handle {:?} [{:#010X}]",
                archive_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_open_directory(
    archive_handle: FS_Archive,
    path: FS_Path,
) -> Result<Handle, IoError> {
    let mut directory_handle: Handle = 0;

    let res = unsafe { FSUSER_OpenDirectory(&mut directory_handle, archive_handle, path) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open directory at path {:?} via handle {:?} [{:#010X}]",
                path,
                archive_handle,
                res
            ),
        ));
    }

    Ok(directory_handle)
}

pub(super) fn ctr_read_directory(
    directory_handle: Handle,
) -> Result<Vec<FS_DirectoryEntry>, IoError> {
    let mut entries: Vec<FS_DirectoryEntry> = vec![unsafe { std::mem::zeroed() }; 32];
    let mut entries_read = 0;

    let res = unsafe {
        FSDIR_Read(
            directory_handle,
            &mut entries_read,
            entries.len() as u32,
            entries.as_mut_ptr(),
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not read directory via handle {:?} [{:#010X}]",
                directory_handle,
                res
            ),
        ));
    }

    entries.truncate(entries_read as usize);

    Ok(entries)
}

pub(super) fn ctr_create_directory(
    archive_handle: FS_Archive,
    path: FS_Path,
) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CreateDirectory(archive_handle, path, 0) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not create directory at via handle {:?} at path {:?} [{:#010X}]",
                archive_handle,
                path,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_close_directory(directory_handle: Handle) -> Result<(), IoError> {
    let res = unsafe { FSDIR_Close(directory_handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close directory via handle {:?} [{:010X}]",
                directory_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_open_file(
    archive_handle: FS_Archive,
    path: FS_Path,
    flags: u8,
) -> Result<Handle, IoError> {
    let mut handle: Handle = 0;

    let res = unsafe { FSUSER_OpenFile(&mut handle, archive_handle, path, flags as u32, 0) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open file via handle {:?} at path {:?} [{:#010X}]",
                archive_handle,
                path,
                res
            ),
        ));
    }

    Ok(handle)
}

pub(super) fn ctr_read_file(
    file_handle: Handle,
    offset: u64,
    length: u64,
) -> Result<Vec<u8>, IoError> {
    let mut buffer = vec![0u8; length as usize];
    let mut bytes_read: u32 = 0;

    let res = unsafe {
        FSFILE_Read(
            file_handle,
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
                "could not read bytes {} to {} via handle {:?} [{:#010X}]",
                offset,
                offset + length,
                file_handle,
                res
            ),
        ));
    }

    if bytes_read != length as u32 {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "wrong amount of bytes were read ({}/{}) via handle {:?} [{:#010X}]",
                length,
                bytes_read,
                file_handle,
                res
            ),
        ));
    }

    Ok(buffer)
}

pub(super) fn ctr_write_file(
    file_handle: Handle,
    offset: u64,
    buffer: &[u8],
    flags: u16,
) -> Result<(), IoError> {
    let mut bytes_written: u32 = 0;

    let res = unsafe {
        FSFILE_Write(
            file_handle,
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
                "could not write source buffer via handle {:?} [{:#010X}]",
                file_handle,
                res
            ),
        ));
    }

    if bytes_written != buffer.len() as u32 {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "wrong amount of bytes were written ({}/{}) via handle {:?} [{:#010X}]",
                buffer.len(),
                bytes_written,
                file_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_create_file(
    archive_handle: FS_Archive,
    path: FS_Path,
    size: u64,
) -> Result<(), IoError> {
    let res = unsafe { FSUSER_CreateFile(archive_handle, path, 0, size) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not create file via handle {:?} at path {:?} [{:#010X}]",
                archive_handle,
                path,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_close_file(file_handle: Handle) -> Result<(), IoError> {
    let res = unsafe { FSFILE_Close(file_handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close file via handle {:?} [{:010X}]",
                file_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_delete_file(archive_handle: FS_Archive, path: FS_Path) -> Result<(), IoError> {
    let res = unsafe { FSUSER_DeleteFile(archive_handle, path) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not delete file via handle {:?} at path {:?} [{:010X}]",
                archive_handle,
                path,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_get_file_size(file_handle: Handle) -> Result<u64, IoError> {
    let mut output = 0;
    let res = unsafe { FSFILE_GetSize(file_handle, &mut output) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not get size of file via handle {:?} [{:010X}]",
                file_handle,
                res
            ),
        ));
    }

    Ok(output)
}

pub(super) fn ctr_set_file_size(file_handle: Handle, size: u64) -> Result<(), IoError> {
    let res = unsafe { FSFILE_SetSize(file_handle, size) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not set size of file via handle {:?} to {} bytes [{:010X}]",
                file_handle,
                size,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_commit_archive(archive_handle: FS_Archive) -> Result<(), IoError> {
    let res = unsafe {
        FSUSER_ControlArchive(
            archive_handle,
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
                "could not commit archive via handle {:?} [{:#010X}]",
                archive_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn ctr_reset_secure_save_meta(title_id: u64) -> Result<(), IoError> {
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

pub(super) fn ctr_getr_ext_data_id_for_title(title_id: u64) -> Result<u64, IoError> {
    let mut extdata_id: u64 = 0;

    let res = unsafe { AM_GetTitleExtDataId(&mut extdata_id, MediaType::Sd as u8, title_id) };

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

    Ok(extdata_id)
}
