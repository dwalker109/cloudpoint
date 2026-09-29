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

    // Slot 0 valid, check slot 1 directly as well
    if let Some((save_size, times_saved_s0)) = slot_at(0) {
        let offset_s1 = HEADER_SIZE + save_size;

        let winning_offset = match slot_at(offset_s1) {
            Some((_, times_saved_s1)) if times_saved_s1 > times_saved_s0 => offset_s1,
            _ => 0,
        };

        return Some((winning_offset, save_size));
    }

    // Slot 0 not valid, iterate potential offsets for slot 1
    for save_size in SAVE_SIZES {
        let offset = HEADER_SIZE + save_size;

        if slot_at(offset).is_some() {
            return Some((offset, save_size));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Read};

    #[test]
    fn selects_slot_0() {
        let mut buf = Vec::new();
        File::open("./fixtures/agb_0.bin")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();

        let (offset, save_size) = locate_current(&buf).unwrap();
        assert_eq!(offset, 0x0000);
        assert_eq!(save_size, 0x2000);
    }

    #[test]
    fn selects_slot_1() {
        let mut buf = Vec::new();
        File::open("./fixtures/agb_1.bin")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();

        let (offset, save_size) = locate_current(&buf).unwrap();
        assert_eq!(offset, 0x2200);
        assert_eq!(save_size, 0x2000);
    }

    #[test]
    fn fallback_slot_1() {
        let mut buf = Vec::new();
        File::open("./fixtures/agb_2.bin")
            .unwrap()
            .read_to_end(&mut buf)
            .unwrap();

        let (offset, save_size) = locate_current(&buf).unwrap();
        assert_eq!(offset, 0x2200);
        assert_eq!(save_size, 0x2000);
    }
}
