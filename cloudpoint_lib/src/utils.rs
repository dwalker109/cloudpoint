pub fn decode_utf16(bytes: &[u8]) -> String {
    let u16 = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|&c| c != 0)
        .collect::<Vec<u16>>();

    String::from_utf16_lossy(&u16)
}

#[cfg(test)]
mod tests {
    #[test]
    fn decode_utf16() {
        let bytes = [
            0x48, 0x00, 0x65, 0x00, 0x6c, 0x00, 0x6c, 0x00, 0x6f, 0x00, 0x01, 0x9c, 0x00, 0x00,
        ];
        let result = super::decode_utf16(&bytes);
        assert_eq!(result, "Hello鰁");
    }
}
