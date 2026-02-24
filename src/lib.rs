use std::{collections::HashMap, ffi::c_void, io::Cursor};

use chunktree::tree::{Leaf, Tree, TreeError};
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    FS_ATTRIBUTE_DIRECTORY, FS_Archive, FS_DirectoryEntry, FS_OPEN_CREATE, FS_OPEN_READ,
    FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, FSDIR_Read, FSFILE_Close, FSFILE_GetSize, FSFILE_Read,
    FSFILE_SetSize, FSFILE_Write, FSUSER_CloseArchive, FSUSER_DeleteFile, FSUSER_OpenArchive,
    FSUSER_OpenDirectory, FSUSER_OpenFile, PATH_BINARY, PATH_UTF16, R_FAILED, R_SUCCEEDED, fsInit,
    fsMakePath,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CtruUserSaveArchive {
    pub title_id: u64,
    pub archive: FS_Archive,
}

impl CtruUserSaveArchive {
    pub fn open(title_id: u64) -> Self {
        unsafe { fsInit() };

        let mut archive: FS_Archive = 0;

        unsafe {
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

            if (R_FAILED(res)) {
                panic!("Could not open archive for {}", title_id);
            }
        }

        Self { title_id, archive }
    }
}

// impl Drop for CtruUserSaveArchive {
//     fn drop(&mut self) {
//         unsafe {
//             let res = FSUSER_CloseArchive(self.title_id);

//             if (R_FAILED(res)) {
//                 panic!("Could not close archive for {}", self.title_id)
//             }
//         }
//     }
// }

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveFileLeaf {
    path_utf16: <ArchiveFileLeaf as Leaf>::Id,
    path_ascii: String,
    ctx: <ArchiveFileLeaf as Leaf>::Context,
}

impl Leaf for ArchiveFileLeaf {
    type Id = Vec<u16>;

    type Context = CtruUserSaveArchive;

    fn new(id: &Self::Id, ctx: &Self::Context) -> Result<Self, TreeError> {
        let path_utf16 = id.clone();
        let path_ascii = String::from_utf16_lossy(&path_utf16);

        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, path_utf16.as_ptr() as *const _);

            let res = FSUSER_OpenFile(
                &mut handle,
                ctx.archive,
                data,
                FS_OPEN_CREATE as u32 | FS_OPEN_WRITE as u32,
                0,
            );

            if R_FAILED(res) {
                panic!("Failed to open {:?}", id);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", id);
            }
        }

        Ok(Self {
            path_utf16,
            path_ascii,
            ctx: ctx.clone(),
        })
    }

    fn pad(&mut self, length: u64) -> Result<(), TreeError> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, self.path_utf16.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_WRITE as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path_utf16);
            }

            let res = FSFILE_SetSize(handle, length);

            if R_FAILED(res) {
                panic!(
                    "Failed to set size of {:?} to {} bytes",
                    self.path_utf16, length
                );
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path_utf16);
            }
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), TreeError> {
        unsafe {
            let data = fsMakePath(PATH_UTF16, self.path_utf16.as_ptr() as *const _);

            let res = FSUSER_DeleteFile(self.ctx.archive, data);

            if R_FAILED(res) {
                panic!("Failed to delete {:?}", self.path_utf16);
            }
        }

        Ok(())
    }

    fn id(&self) -> &Self::Id {
        &self.path_utf16
    }

    fn data(&self) -> Result<impl std::io::Read + std::io::Seek, TreeError> {
        let mut handle = 0;
        let mut buf = vec![0u8; 0];

        unsafe {
            let data = fsMakePath(PATH_UTF16, self.path_utf16.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path_utf16);
            }

            let mut size: u64 = 0;
            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                panic!("Failed to get size in bytes of {:?}", self.path_utf16);
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
                panic!("Failed to read contents of {:?}", self.path_utf16);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path_utf16);
            }
        }

        Ok(Cursor::new(buf))
    }

    fn length(&self) -> Result<u64, TreeError> {
        let mut handle = 0;
        let mut size: u64 = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, self.path_utf16.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path_utf16);
            }

            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                panic!("Failed to get size in bytes of {:?}", self.path_utf16);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path_utf16);
            }
        }

        Ok(size)
    }

    fn write_chunk(
        &mut self,
        offset: u64,
        length: u64,
        source: &mut impl std::io::Read,
    ) -> Result<(), TreeError> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, self.path_utf16.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path_utf16);
            }

            let mut buf = vec![0x00; length as usize];
            source.read_exact(&mut buf)?;

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
                panic!("Failed to write source buffer to {:?}", self.path_utf16);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path_utf16);
            }
        }

        Ok(())
    }
}

pub fn walk_tree(title_id: u64) -> Tree<ArchiveFileLeaf> {
    let ctx = CtruUserSaveArchive::open(title_id);
    let mut results = HashMap::new();

    walk_sub("/\0".encode_utf16().collect(), &ctx, &mut results);

    fn walk_sub(
        dir_path: <ArchiveFileLeaf as Leaf>::Id,
        ctx: &<ArchiveFileLeaf as Leaf>::Context,
        results: &mut HashMap<<ArchiveFileLeaf as Leaf>::Id, ArchiveFileLeaf>,
    ) {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_UTF16, dir_path.as_ptr() as *const _);

            let res = FSUSER_OpenDirectory(&mut handle, ctx.archive, data);

            if R_FAILED(res) {
                println!(
                    "Failed to open directory {:?}",
                    String::from_utf16_lossy(&dir_path)
                );

                return;
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
                panic!("Failed to list directory {:?}", dir_path);
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
                    let leaf = ArchiveFileLeaf::new(&fq_path, ctx).expect("leaf created");
                    results.insert(fq_path, leaf);
                }
            }
        }
    }

    Tree::new(results, ctx)
}

mod web {
    
}