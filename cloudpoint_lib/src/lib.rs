pub mod ctr;
pub mod http;
pub mod sync;
pub mod version;
pub mod utils {
    pub fn decode_utf16(bytes: &[u8]) -> String {
        let u16 = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|&c| c != 0)
            .collect::<Vec<u16>>();

        String::from_utf16_lossy(&u16)
    }
}
