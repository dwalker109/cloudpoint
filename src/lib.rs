use std::{
    collections::HashMap,
    ffi::c_void,
    io::Cursor,
    path::{Path, PathBuf},
};

use chunktree::tree::{Leaf, Tree, TreeError};
use ctru::services::fs::{ArchiveID, MediaType};
use ctru_sys::{
    ARCHIVE_ACTION_COMMIT_SAVE_DATA, FS_ATTRIBUTE_DIRECTORY, FS_Archive, FS_DirectoryEntry,
    FS_OPEN_CREATE, FS_OPEN_READ, FS_OPEN_WRITE, FS_Path, FS_WRITE_FLUSH, FSDIR_Read, FSFILE_Close,
    FSFILE_GetSize, FSFILE_Read, FSFILE_SetSize, FSFILE_Write, FSUSER_CloseArchive,
    FSUSER_ControlArchive, FSUSER_DeleteFile, FSUSER_OpenArchive, FSUSER_OpenDirectory,
    FSUSER_OpenFile, PATH_ASCII, PATH_BINARY, PATH_UTF16, R_FAILED, R_SUCCEEDED, fsInit,
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
    path: String,
    ctx: CtruUserSaveArchive,
}

impl Leaf for ArchiveFileLeaf {
    type Context = CtruUserSaveArchive;

    fn new(path: impl AsRef<Path>, ctx: &Self::Context) -> Result<Self, TreeError> {
        let path = path.as_ref().to_string_lossy().into_owned();
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(
                &mut handle,
                ctx.archive,
                data,
                FS_OPEN_CREATE as u32 | FS_OPEN_WRITE as u32,
                0,
            );

            if R_FAILED(res) {
                panic!("Failed to open {:?}", path);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", path);
            }
        }

        Ok(Self {
            path,
            ctx: ctx.clone(),
        })
    }

    fn pad(&mut self, length: u64) -> Result<(), TreeError> {
        let mut handle = 0;

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_WRITE as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path);
            }

            let res = FSFILE_SetSize(handle, length);

            if R_FAILED(res) {
                panic!("Failed to set size of {:?} to {} bytes", self.path, length);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path);
            }
        }

        Ok(())
    }

    fn delete(&mut self) -> Result<(), TreeError> {
        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_DeleteFile(self.ctx.archive, data);

            if R_FAILED(res) {
                panic!("Failed to delete {:?}", self.path);
            }
        }

        Ok(())
    }

    fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    fn data(&self) -> Result<impl std::io::Read + std::io::Seek, TreeError> {
        let mut handle = 0;
        let mut buf = vec![0u8; 0];

        unsafe {
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_READ as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path);
            }

            let mut size: u64 = 0;
            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                panic!("Failed to get size in bytes of {:?}", self.path);
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
                panic!("Failed to read contents of {:?}", self.path);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path);
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
                panic!("Failed to open {:?}", self.path);
            }

            let res = FSFILE_GetSize(handle, &mut size);

            if R_FAILED(res) {
                panic!("Failed to get size in bytes of {:?}", self.path);
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path);
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
            let data = fsMakePath(PATH_ASCII, self.path.as_ptr() as *const _);

            let res = FSUSER_OpenFile(&mut handle, self.ctx.archive, data, FS_OPEN_WRITE as u32, 0);

            if R_FAILED(res) {
                panic!("Failed to open {:?}", self.path);
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
                panic!(
                    "Failed to write source buffer to {:?} with {:x}",
                    self.path, res
                );
            }

            if bytes_written != length as u32 {
                panic!("Short write, {bytes_written}/{length}")
            }

            let res = FSFILE_Close(handle);

            if R_FAILED(res) {
                panic!("Failed to close {:?}", self.path);
            }

            let res = FSUSER_ControlArchive(
                self.ctx.archive,
                ARCHIVE_ACTION_COMMIT_SAVE_DATA,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            );

            if R_FAILED(res) {
                panic!("Failed to commit archive {}", self.ctx.archive);
            }
        }

        Ok(())
    }
}

pub fn walk_tree(title_id: u64) -> Tree<ArchiveFileLeaf> {
    let ctx = CtruUserSaveArchive::open(title_id);
    let mut results = HashMap::new();

    walk_sub("/\0".encode_utf16().collect(), &ctx, &mut results);

    /// Note utf16 paths used here and converted when inserted to HashMap
    fn walk_sub(
        dir_path: Vec<u16>,
        ctx: &<ArchiveFileLeaf as Leaf>::Context,
        results: &mut HashMap<PathBuf, ArchiveFileLeaf>,
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
                    let fq_path = PathBuf::from(String::from_utf16_lossy(&fq_path));
                    let leaf = ArchiveFileLeaf::new(&fq_path, ctx).expect("leaf created");
                    results.insert(fq_path, leaf);
                }
            }
        }
    }

    Tree::new(results, ctx)
}

pub mod web {
    use std::{
        io::{self, BufRead, BufReader, Cursor, Read, Write},
        net::TcpStream,
        time::Duration,
    };

    use chunktree::{
        store::{StoreRead, StoreWrite},
        version::Version,
    };

    use crate::ArchiveFileLeaf;

    pub struct HttpStore;

    impl StoreRead for HttpStore {
        fn get_chunk(&self, hash: u64) -> Result<impl Read, chunktree::store::StoreError> {
            let mut stream = TcpStream::connect("192.168.1.45:8080").unwrap();

            let request = format!(
                "GET /game/chunks/{hash} HTTP/1.1\r\n\
                 Host: Cloudpoint\r\n\
                 Connection: close\r\n\
                 \r\n"
            );

            stream.write_all(request.as_bytes()).unwrap();

            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();

            // crude HTTP split (no chunked encoding support)
            let body = response
                .split(|b| b"\r\n\r\n".contains(b))
                .collect::<Vec<_>>();

            let body = response
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|i| response[i + 4..].to_vec())
                .unwrap();

            Ok(Cursor::new(body))
        }
    }

    impl StoreWrite for HttpStore {
        fn put_chunk(
            &mut self,
            hash: u64,
            data: &mut impl std::io::Read,
            length: u64,
        ) -> Result<(), chunktree::store::StoreError> {
            let mut stream = TcpStream::connect("192.168.1.45:8080").unwrap();

            // HEAD request
            let head = format!(
                "HEAD /game/chunks/{hash} HTTP/1.1\r\nHost: 192.168.1.45:8080\r\nConnection: keep-alive\r\n\r\n"
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.flush().unwrap();

            // Read HEAD response
            let mut reader = BufReader::new(stream);
            let mut status_line = String::new();
            reader.read_line(&mut status_line).unwrap();

            // Drain remaining headers
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }

            // Check status
            if status_line.contains("200") {
                println!("File already exists, skipping.");
                return Ok(());
            }

            // Get stream back out of BufReader
            let mut stream = reader.into_inner();

            // PUT on the same connection
            let put_headers = format!(
                "PUT /game/chunks/{hash} HTTP/1.1\r\nHost: 192.168.1.45:8080\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n",
            );
            stream.write_all(put_headers.as_bytes()).unwrap();
            io::copy(data, &mut stream).unwrap();
            stream.flush().unwrap();

            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            // println!("PUT response: {}", String::from_utf8_lossy(&response));
            println!("File {hash} uploaded.");

            Ok(())
        }
    }

    pub fn upload_version(
        title_id: u64,
        version: &Version<ArchiveFileLeaf>,
    ) -> Result<(), chunktree::store::StoreError> {
        let mut stream = TcpStream::connect("192.168.1.45:8080").unwrap();

        let data = postcard::to_allocvec(&version).unwrap();

        let put_headers = format!(
            "PUT /game/versions/saves/{}.{}.ver HTTP/1.1\r\nHost: 192.168.1.45:8080\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            title_id,
            version.fingerprint(),
            data.len()
        );

        stream.write_all(put_headers.as_bytes()).unwrap();
        stream.write_all(&data).unwrap();
        stream.flush().unwrap();

        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        println!("PUT response: {}", String::from_utf8_lossy(&response));

        Ok(())
    }

    pub fn download_version(
        title_id: u64,
    ) -> Result<Version<ArchiveFileLeaf>, chunktree::store::StoreError> {
        let mut stream = TcpStream::connect("192.168.1.45:8080").unwrap();

        // let json = serde_json::to_string(&version).unwrap();

        let get_headers = format!(
            "GET /game/versions/saves/foo.json HTTP/1.1\r\nHost: 192.168.1.45:8080\r\nConnection: close\r\n\r\n",
        );

        stream.write_all(get_headers.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let body = response.split("\r\n\r\n").nth(1).unwrap_or("");
        let version = serde_json::from_str::<Version<ArchiveFileLeaf>>(body).unwrap();

        Ok(version)
    }
}
