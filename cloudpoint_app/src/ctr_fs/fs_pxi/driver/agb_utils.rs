//* See https://www.3dbrew.org/wiki/3DS_Virtual_Console#NAND_Savegame

use anyhow::anyhow;
use sha2::{Digest, Sha256};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};

pub const HEADER_SIZE: usize = 0x200;
pub const CMAC_SIZE: usize = 0x10;
pub const MAGIC: &[u8; 4] = b".SAV";
pub const SAVE_SIZES: [usize; 5] = [0x200, 0x2000, 0x8000, 0x1_0000, 0x2_0000];

const OFFSET_MAGIC: usize = 0x00;
const OFFSET_AES_CMAC: usize = 0x10;
const OFFSET_AES_CMAC_BEGIN: usize = 0x30;
const OFFSET_TIMES_SAVED: usize = 0x34;
const OFFSET_SAVE_START_ADDR: usize = 0x50;
const OFFSET_SAVE_SIZE: usize = 0x54;

pub fn extract_savedata(container: &[u8]) -> Result<&[u8], IoError> {
    let (offset, save_size) = locate_current(container).ok_or_else(|| {
        IoError::new(
            IoErrorKind::Other,
            anyhow!("could not extract a valid agb save"),
        )
    })?;

    let body_offset = offset + HEADER_SIZE;

    Ok(&container[body_offset..body_offset + save_size])
}

pub fn prepare_write_parts(
    container: &[u8],
    new_savedata: &[u8],
) -> Result<(u64, u64, [u8; HEADER_SIZE], [u8; 32]), IoError> {
    let (offset, ..) = locate_current(container).ok_or_else(|| {
        IoError::new(
            IoErrorKind::Other,
            anyhow!("could not extract a valid agb save",),
        )
    })?;

    let mut header = [0u8; HEADER_SIZE];
    header.copy_from_slice(&container[offset..offset + HEADER_SIZE]);

    let hash = Sha256::new()
        .chain_update(&header[OFFSET_AES_CMAC_BEGIN..])
        .chain_update(new_savedata)
        .finalize()
        .into();

    Ok((offset as u64, (offset + HEADER_SIZE) as u64, header, hash))
}

pub fn update_header_cmac(header: &mut [u8; HEADER_SIZE], cmac: [u8; CMAC_SIZE]) {
    header[OFFSET_AES_CMAC..OFFSET_AES_CMAC + CMAC_SIZE].copy_from_slice(&cmac);
}

fn locate_current(container: &[u8]) -> Option<(usize, usize)> {
    let slot_at = |offset: usize| -> Option<(usize, u32)> {
        let header = container.get(offset..offset + HEADER_SIZE)?;

        let read_u32 = |offset| -> u32 {
            u32::from_le_bytes(
                header[offset..offset + 4]
                    .try_into()
                    .expect("u32 from 4 byte slice"),
            )
        };

        if &header[OFFSET_MAGIC..4] != MAGIC
            || read_u32(OFFSET_SAVE_START_ADDR) as usize != HEADER_SIZE
        {
            return None;
        }

        let save_size = read_u32(OFFSET_SAVE_SIZE) as usize;
        if !SAVE_SIZES.contains(&save_size) {
            return None;
        }

        let data_start = offset + HEADER_SIZE;
        container.get(data_start..data_start + save_size)?;

        Some((save_size, read_u32(OFFSET_TIMES_SAVED)))
    };

    // Top slot is valid, inspect directly
    if let Some((save_size, times_saved_top)) = slot_at(0) {
        let bottom_offset = HEADER_SIZE + save_size;

        let offset = match slot_at(bottom_offset) {
            Some((_, times_saved_bottom)) if times_saved_bottom > times_saved_top => bottom_offset,
            _ => 0,
        };

        return Some((offset, save_size));
    }

    // Top slot not valid, iterate potential offsets for bottom slot
    for save_size in SAVE_SIZES {
        let offset = HEADER_SIZE + save_size;

        if slot_at(offset).is_some() {
            return Some((offset, save_size));
        }
    }

    None
}
