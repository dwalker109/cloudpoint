use anyhow::Result;
use cloudpoint_lib::sync::SyncItem;
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{FS_DirectoryEntry, FS_Path, Handle, PATH_ASCII, PATH_BINARY, fsMakePath};
// use ffi::{
//     ctr_close_archive, ctr_close_directory, ctr_close_file, ctr_commit_archive,
//     ctr_create_directory, ctr_create_file, ctr_delete_file, ctr_get_file_size, ctr_open_archive,
//     ctr_open_directory, ctr_open_file, ctr_read_directory, ctr_read_ext_smdh, ctr_read_file,
//     ctr_read_title_smdh, ctr_reset_secure_save_meta, ctr_set_file_size, ctr_write_file,
// };
use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::io::{self, Error as IoError, Read, Seek, SeekFrom};

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
    archive_handle: u64,
    buffer: RefCell<Vec<u8>>,
}

impl FsPxiArchive {
    pub fn open(sync_item: SyncItem) -> Result<Self, IoError> {
        log::debug!("opening archive for {}", sync_item);

        let path = FsPxiArchivePath::new(sync_item)?;
        let handle = ctr_open_archive(path.archive_id, path.fs_path())?;

        // open the underlying file and grab the bytes, file can be allowed to drop after

        Ok(Self {
            sync_item,
            archive_handle: handle,
        })
    }

    pub fn sync_item(&self) -> &SyncItem {
        &self.sync_item
    }

    // pub fn open_file(&self, path: &FsUserInnerPath, flags: u8) -> Result<FsUserInnerFile, IoError> {
    //     log::debug!("opening file {:?} in archive for {}", path, self.sync_item);

    //     let file_handle = ctr_open_file(self.archive_handle, path.fs_path(), flags)?;

    //     Ok(FsUserInnerFile { file_handle })
    // }

    pub fn buffer_to_vec(&self) -> Vec<u8> {
        self.buffer.borrow().to_vec()
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.borrow().len()
    }

    pub fn buffer_write(&self, offset: u64, source: &mut impl io::Read) {
        let start = offset as usize;
        let end = start+self.buffer_len();

        io::copy(source, &mut self.buffer.borrow_mut()[start..end]);
    }

    pub fn finalise(&self) -> Result<(), IoError> {
        log::debug!("finalising save write in archive for {}", self.sync_item);

        // open the underlying file and write the bytes, file can be allowed to drop after

        todo!()

        Ok(())
    }
}

impl Drop for FsPxiArchive {
    fn drop(&mut self) {
        log::debug!("dropping archive for {}", self.sync_item);
        ctr_close_archive(self.archive_handle).expect("archive should be closable");
    }
}

pub struct FsPxiInnerFile {
    file_handle: Handle,
}

impl FsPxiInnerFile {
    pub fn read_all(&self) -> Result<Vec<u8>, IoError> {
        todo!()
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
}

impl Drop for FsPxiInnerFile {
    fn drop(&mut self) {
        log::debug!("dropping handle {}", self.file_handle);
        ctr_close_file(self.file_handle).expect("file should be closable");
    }
}
