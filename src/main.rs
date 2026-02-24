use std::fs::{File, create_dir_all};
use std::mem;
use std::time::SystemTime;

use chunktree::store::LocalFileStore;
use chunktree::version::Version;
use cloudpoint::walk_tree;
use ctru::prelude::*;
use ctru::services::am::Am;
use ctru::services::fs;
use ctru::services::fs::MediaType;
use ctru_sys::*;
use std::ffi::{CString, c_void};
use std::io::{BufWriter, Write};

fn main() {
    unsafe {
        ctru_sys::link3dsConnectToHost(true, true);
    }

    std::panic::set_hook(Box::new(|info| {
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/3ds/Cloudpoint/panic.txt")
            .unwrap();
        writeln!(log, "{}", info).ok();
    }));

    let gfx = Gfx::new().expect("Couldn't obtain GFX controller");
    let mut hid = Hid::new().expect("Couldn't obtain HID controller");
    let apt = Apt::new().expect("Couldn't obtain APT controller");

    let top_screen = Console::new(gfx.top_screen.borrow_mut());
    let bottom_screen = Console::new(gfx.bottom_screen.borrow_mut());

    // Dir setup
    create_dir_all("sdmc:/3ds/Cloudpoint").ok();

    // Set up the AM service to retrieve the wanted information.
    let am = Am::new().expect("Couldn't obtain AM controller");

    // Amount of titles installed on the SD card.
    let sd_count = am
        .title_count(MediaType::Sd)
        .expect("Failed to get sd title count");
    // List of titles installed on the SD card.
    let sd_list = am
        .title_list(MediaType::Sd)
        .expect("Failed to get sd title list");

    let mut offset = 0;
    let mut refresh = true;

    while apt.main_loop() {
        hid.scan_input();

        if hid.keys_down().contains(KeyPad::START) {
            break;
        }

        let cur_list = &sd_list;
        let mut selected_title = cur_list.get(offset).unwrap();

        if hid.keys_down().intersects(KeyPad::DOWN) {
            if offset + 1 < cur_list.len() {
                offset += 1;
                refresh = true;
            }
        } else if hid.keys_down().intersects(KeyPad::UP) && offset > 0 {
            offset -= 1;
            refresh = true;
        } else if hid.keys_down().intersects(KeyPad::A) {
            let tree = walk_tree(selected_title.id());
            let version = Version::new(&tree, 64, 256, 1024).unwrap();
            let mut store =
                LocalFileStore::new(format!("/3ds/Cloudpoint/{}/store", selected_title.id()))
                    .unwrap();
            version.copy_chunks(&tree, &mut store).unwrap();
            let out = File::create(format!(
                "/3ds/Cloudpoint/{}/{}.json",
                selected_title.id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            ))
            .unwrap();
            let w = BufWriter::new(out);
            serde_json::to_writer_pretty(w, &version).ok();
        }

        // Render the title list via a scrollable text UI.
        if refresh {
            // Clear the top screen and write title IDs to it.
            top_screen.select();
            print!("\x1b[2J");

            // Top screen has 30 rows.
            for (i, title) in cur_list.iter().skip(offset).take(29).enumerate() {
                if i == 0 {
                    selected_title = title;
                    println!("=> {:x}", title.id());
                } else {
                    println!("   {:x}", title.id());
                }
            }

            // Clear the bottom screen and write the properties of selected title to it.
            bottom_screen.select();
            bottom_screen.clear();
            println!("Press Start to exit");

            // Move cursor to top left.
            println!("\x1b[1;1");

            println!("Size: {} kB", selected_title.size() / 1024);
            println!("Version: 0x{:x}", selected_title.version());
            println!("Product code: \"{}\"", selected_title.product_code());
            println!("Title count: {sd_count}");

            // Read SMDH
            unsafe { read_smdh(selected_title.id()) };

            // Read save
            unsafe { read_save(selected_title.id()) };

            refresh = false;
        }

        gfx.wait_for_vblank();
    }
}

unsafe fn read_save(title_id: u64) {
    unsafe { fsInit() };

    let archive_path_data: [u32; 3] = [
        MediaType::Sd as u32,
        title_id as u32,
        (title_id >> 32) as u32,
    ];

    let archive_path = FS_Path {
        type_: PATH_BINARY,
        size: (archive_path_data.len() * 4) as u32,
        data: archive_path_data.as_ptr() as *const c_void,
    };

    let mut archive: FS_Archive = 0;

    let res = unsafe {
        FSUSER_OpenArchive(
            &mut archive,
            fs::ArchiveID::UserSavedata as u32,
            archive_path,
        )
    };

    if R_SUCCEEDED(res) {
        dbg!(archive);
    } else {
        println!("Could not open save archive");
        println!("0x{:08X}", res);
        unsafe { fsExit() }

        return;
    }

    let mut root_dir: Handle = 0;
    let res = unsafe {
        FSUSER_OpenDirectory(
            &mut root_dir,
            archive,
            FS_Path {
                type_: PATH_ASCII,
                size: 2,
                data: CString::new("/").unwrap().as_ptr() as *const c_void,
            },
        )
    };

    if R_SUCCEEDED(res) {
        dbg!(root_dir);
    } else {
        println!("Could not open root dir");
        println!("0x{:08X}", res);
        FSUSER_CloseArchive(archive);
        unsafe { fsExit() }

        return;
    }

    let mut qty = 0u32;
    let max = 8u32;
    let mut entries: Vec<FS_DirectoryEntry> = vec![mem::zeroed(); max as usize];
    let res = unsafe { FSDIR_Read(root_dir, &mut qty, max, entries.as_mut_ptr()) };

    if R_SUCCEEDED(res) {
        for e in entries
            .iter()
            .map(|e| {
                String::from_utf16_lossy(
                    e.name
                        .iter()
                        .take_while(|&&c| c != 0)
                        .copied()
                        .collect::<Vec<u16>>()
                        .as_ref(),
                )
            })
            .filter(|e| !e.is_empty())
        {
            println!("F: {e}");
        }
    } else {
        FSUSER_CloseArchive(archive);
        fsExit();

        return;
    }

    unsafe {
        FSDIR_Close(root_dir);
        FSUSER_CloseArchive(archive);
        fsExit();
    }
}

unsafe fn read_smdh(title_id: u64) {
    let file_path_data: [u32; 5] = [0x00000000, 0x00000000, 0x00000002, 0x6E6F6369, 0x00000000];
    let archive_path_data: [u32; 4] = [
        title_id as u32,
        (title_id >> 32) as u32,
        MediaType::Sd as u32,
        0x00000000,
    ];

    let file_path = FS_Path {
        type_: PATH_BINARY,
        size: (file_path_data.len() * 4) as u32,
        data: file_path_data.as_ptr() as *const c_void,
    };
    let archive_path = FS_Path {
        type_: PATH_BINARY,
        size: (archive_path_data.len() * 4) as u32,
        data: archive_path_data.as_ptr() as *const c_void,
    };

    let mut file_handle: Handle = 0;

    let res = unsafe {
        FSUSER_OpenFileDirectly(
            &mut file_handle,
            fs::ArchiveID::SaveDataAndContent as u32,
            archive_path,
            file_path,
            FS_OPEN_READ as u32,
            0u32,
        )
    };

    if R_SUCCEEDED(res) {
        let mut smdh_buf = [0u8; 0x36c0];
        let mut bytes_read: u32 = 0;

        let res = unsafe {
            FSFILE_Read(
                file_handle,
                &mut bytes_read,
                0u64,
                smdh_buf.as_mut_ptr() as *mut c_void,
                0x36c0,
            )
        };

        if R_SUCCEEDED(res) {
            println!("File successfully read");
            if &smdh_buf[0..4] == b"SMDH" {
                println!("SMDH read successfully!");

                let _magic = String::from_utf8_lossy(&smdh_buf[0x00..0x04]);
                let _version = String::from_utf8_lossy(&smdh_buf[0x04..0x06]);
                let title_en_short = title_from_utf16_bytes(&smdh_buf[0x208..0x288]);
                let title_en_long = title_from_utf16_bytes(&smdh_buf[0x288..0x388]);
                let title_en_pub = title_from_utf16_bytes(&smdh_buf[0x388..0x408]);

                println!("Short: {title_en_short}");
                println!("Long: {title_en_long}");
                println!("Publisher: {title_en_pub}");
            }
        } else {
            println!("Failed to read SMDH: {res:x}");
        }
    } else {
        println!("Failed to open file!");
    }

    unsafe {
        let _ = FSFILE_Close(file_handle);
    };
}

fn title_from_utf16_bytes(bytes: &[u8]) -> String {
    // Convert every 2 bytes into u16
    let u16_iter = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]));

    // Decode UTF-16
    String::from_utf16(&u16_iter.filter(|b| *b != 0x00).collect::<Vec<u16>>())
        .expect("Invalid UTF-16 data")
}
