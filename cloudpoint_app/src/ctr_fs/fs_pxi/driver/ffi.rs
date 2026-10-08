use anyhow::anyhow;
use ctru_sys::{
    ARCHIVE_SAVEDATA_AND_CONTENT, FS_Path, FSPXI_Archive, FSPXI_CalcSavegameMAC,
    FSPXI_CloseArchive, FSPXI_CloseFile, FSPXI_File, FSPXI_GetFileSize, FSPXI_OpenArchive,
    FSPXI_OpenFile, FSPXI_ReadFile, FSPXI_WriteFile, PATH_BINARY, R_FAILED,
};
use std::{
    io::{Error as IoError, ErrorKind as IoErrorKind},
    os::raw::c_void,
};

use crate::pxi::PxiSession;

pub(super) fn pxi_open_archive(path: FS_Path) -> Result<FSPXI_Archive, IoError> {
    let mut archive_handle: FSPXI_Archive = 0;

    let res = unsafe {
        FSPXI_OpenArchive(
            PxiSession::handle()?,
            &mut archive_handle,
            ARCHIVE_SAVEDATA_AND_CONTENT,
            path,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open pxi archive at path {:?} [{:#010X}]",
                path,
                res
            ),
        ));
    }

    Ok(archive_handle)
}

pub(super) fn pxi_close_archive(archive_handle: FSPXI_Archive) -> Result<(), IoError> {
    let res = unsafe { FSPXI_CloseArchive(PxiSession::handle()?, archive_handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close pxi archive via handle {:?} [{:#010X}]",
                archive_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn pxi_open_file(
    archive_handle: FSPXI_Archive,
    flags: u32,
) -> Result<FSPXI_File, IoError> {
    static AGB_CONTAINER: [u32; 5] = [1, 1, 3, 0, 0];
    let agb_path = FS_Path {
        type_: PATH_BINARY,
        size: 20,
        data: AGB_CONTAINER.as_ptr() as *const c_void,
    };

    let mut file_handle: FSPXI_File = 0;

    let res = unsafe {
        FSPXI_OpenFile(
            PxiSession::handle()?,
            &mut file_handle,
            archive_handle,
            agb_path,
            flags,
            0,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not open pxi file via handle {:?} at path {:?} [{:#010X}]",
                archive_handle,
                agb_path,
                res
            ),
        ));
    }

    Ok(file_handle)
}

pub(super) fn pxi_close_file(file_handle: FSPXI_File) -> Result<(), IoError> {
    let res = unsafe { FSPXI_CloseFile(PxiSession::handle()?, file_handle) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not close pxi file via handle {:?} [{:#010X}]",
                file_handle,
                res
            ),
        ));
    }

    Ok(())
}

pub(super) fn pxi_get_file_size(file_handle: FSPXI_File) -> Result<u64, IoError> {
    let mut size: u64 = 0;
    let res = unsafe { FSPXI_GetFileSize(PxiSession::handle()?, file_handle, &mut size) };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not get size of pxi file via handle {:?} [{:#010X}]",
                file_handle,
                res
            ),
        ));
    }

    Ok(size)
}

pub(super) fn pxi_read_file(
    file_handle: FSPXI_File,
    offset: u64,
    buf: &mut [u8],
) -> Result<u64, IoError> {
    let mut bytes_read: u32 = 0;

    let res = unsafe {
        FSPXI_ReadFile(
            PxiSession::handle()?,
            file_handle,
            &mut bytes_read,
            offset,
            buf.as_mut_ptr() as *mut _,
            buf.len() as u32,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not read pxi file via handle {:?} at offset {} ({}/{}) [{:#010X}]",
                file_handle,
                offset,
                bytes_read,
                buf.len(),
                res
            ),
        ));
    }

    if bytes_read as usize != buf.len() {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "truncated read ({}/{}) [{:#010X}]",
                bytes_read,
                buf.len(),
                res,
            ),
        ));
    }

    Ok(bytes_read as u64)
}

pub(super) fn pxi_write_file(
    file_handle: FSPXI_File,
    offset: u64,
    buf: &[u8],
    flags: u32,
) -> Result<(), IoError> {
    let mut bytes_written: u32 = 0;

    let res = unsafe {
        FSPXI_WriteFile(
            PxiSession::handle()?,
            file_handle,
            &mut bytes_written,
            offset,
            buf.as_ptr() as *const _,
            buf.len() as u32,
            flags,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not write pxi file via handle {:?} at offset {} [{:#010X}]",
                file_handle,
                offset,
                res
            ),
        ));
    }

    if bytes_written != buf.len() as u32 {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "wrong amount of bytes were written ({}/{}) via pxi handle {:?}",
                bytes_written,
                buf.len(),
                file_handle,
            ),
        ));
    }

    Ok(())
}

pub(super) fn pxi_calc_savegame_mac(
    file_handle: FSPXI_File,
    hash: &[u8; 32],
) -> Result<[u8; 16], IoError> {
    let mut mac = [0u8; 16];

    let res = unsafe {
        FSPXI_CalcSavegameMAC(
            PxiSession::handle()?,
            file_handle,
            hash.as_ptr() as *const _,
            hash.len() as u32,
            mac.as_mut_ptr() as *mut _,
            mac.len() as u32,
        )
    };

    if R_FAILED(res) {
        return Err(IoError::new(
            IoErrorKind::Other,
            anyhow!(
                "could not calculate savegame mac via pxi handle {:?} [{:#010X}]",
                file_handle,
                res
            ),
        ));
    }

    Ok(mac)
}
