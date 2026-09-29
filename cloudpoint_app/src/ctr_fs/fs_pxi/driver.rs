use crate::ctr_fs::fs_pxi::driver::ffi::{
    pxi_calc_savegame_mac, pxi_close_archive, pxi_close_file, pxi_get_file_size, pxi_open_archive,
    pxi_open_file, pxi_read_file, pxi_write_file,
};
use anyhow::Result;
use cloudpoint_lib::sync::SyncItem;
use ctru::services::fs::MediaType;
use ctru_sys::{
    FS_OPEN_READ, FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, FSPXI_Archive, FSPXI_File, PATH_BINARY,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::io::{self, Error as IoError, ErrorKind as IoErrorKind};

mod agb_utils;
mod ffi;

struct FsPxiArchivePath {
    _sync_item: SyncItem,
    buffer: [u32; 4],
}

impl FsPxiArchivePath {
    fn new(sync_item: SyncItem) -> Result<Self, IoError> {
        let buffer = match sync_item {
            SyncItem::Gba(title_id) => [
                title_id as u32,
                (title_id >> 32) as u32,
                MediaType::Sd as u32,
                1,
            ],
            SyncItem::Savedata(_) | SyncItem::Extdata(_) => {
                unreachable!("savedata/extdata items not reached via fs_pxi driver")
            }
        };

        Ok(Self {
            _sync_item: sync_item,
            buffer,
        })
    }

    fn fs_path(&self) -> FS_Path {
        FS_Path {
            type_: PATH_BINARY,
            size: 16,
            data: self.buffer.as_ptr() as *const c_void,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FsPxiArchive {
    sync_item: SyncItem,
    archive_handle: FSPXI_Archive,
    buffer: RefCell<Vec<u8>>,
}

impl FsPxiArchive {
    pub fn open(sync_item: SyncItem) -> Result<Self, IoError> {
        log::debug!("opening archive for {}", sync_item);

        let path = FsPxiArchivePath::new(sync_item)?;
        let handle = pxi_open_archive(path.fs_path())?;
        let archive = Self {
            sync_item,
            archive_handle: handle,
            buffer: RefCell::default(),
        };

        let agb_container = AgbContainer::open(archive.archive_handle, FS_OPEN_READ as u32)?;
        let agb_raw = agb_container.read_all()?;
        let current_data = agb_utils::extract_savedata(&agb_raw)?;
        *archive.buffer.borrow_mut() = current_data.to_vec();

        Ok(archive)
    }

    pub fn sync_item(&self) -> &SyncItem {
        &self.sync_item
    }

    pub fn buffer_to_vec(&self) -> Vec<u8> {
        self.buffer.borrow().to_vec()
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.borrow().len()
    }

    pub fn buffer_write(&self, offset: u64, source: &mut impl io::Read) -> Result<(), IoError> {
        log::debug!("mem only write for {} at offset {}", self.sync_item, offset);

        let mut buffer = self.buffer.borrow_mut();
        let mut dst = &mut buffer[(offset as usize)..];
        io::copy(source, &mut dst)?;

        Ok(())
    }

    pub fn finalise(&self) -> Result<(), IoError> {
        log::debug!("finalising save write in archive for {}", self.sync_item);

        let agb_container =
            AgbContainer::open(self.archive_handle, (FS_OPEN_READ | FS_OPEN_WRITE) as u32)?;
        let agb_raw = agb_container.read_all()?;
        let buffer = self.buffer.borrow();
        let (header_offset, body_offset, mut header, hash) =
            agb_utils::prepare_write_parts(&agb_raw, &buffer)?;

        let cmac = agb_container.calc_mac(&hash)?;
        agb_utils::update_header_cmac(&mut header, cmac);

        agb_container.write(body_offset, &buffer)?;
        agb_container.write(header_offset, &header)?;

        Ok(())
    }
}

impl Drop for FsPxiArchive {
    fn drop(&mut self) {
        log::debug!("dropping archive for {}", self.sync_item);
        pxi_close_archive(self.archive_handle).expect("archive should be closable");
    }
}

struct AgbContainer {
    file_handle: FSPXI_File,
}

impl AgbContainer {
    fn open(archive_handle: FSPXI_Archive, flags: u32) -> Result<Self, IoError> {
        let handle = pxi_open_file(archive_handle, flags)?;

        Ok(Self {
            file_handle: handle,
        })
    }

    fn read_all(&self) -> Result<Vec<u8>, IoError> {
        let size = pxi_get_file_size(self.file_handle)? as usize;
        let mut buf = vec![0u8; size];
        pxi_read_file(self.file_handle, 0, &mut buf[0..])?;

        Ok(buf)
    }

    fn write(&self, offset: u64, buf: &[u8]) -> Result<(), IoError> {
        log::debug!(
            "writing to pxi handle {} at offset {} with length {}",
            self.file_handle,
            offset,
            buf.len()
        );

        pxi_write_file(self.file_handle, offset, buf, FS_WRITE_FLUSH as u32)
    }

    fn calc_mac(&self, hash: &[u8; 32]) -> Result<[u8; agb_utils::CMAC_SIZE], IoError> {
        let mut prev = pxi_calc_savegame_mac(self.file_handle, hash)?;

        for _ in 1..10 {
            let next = pxi_calc_savegame_mac(self.file_handle, hash)?;

            if next == prev {
                return Ok(next);
            }

            prev = next;
        }

        Err(IoError::new(
            IoErrorKind::Other,
            format!("savegame mac did not stabilise after numerous attempts"),
        ))
    }
}

impl Drop for AgbContainer {
    fn drop(&mut self) {
        log::debug!("dropping pxi handle {}", self.file_handle);
        pxi_close_file(self.file_handle).expect("file should be closable");
    }
}
