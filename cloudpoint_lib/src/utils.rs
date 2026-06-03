pub fn decode_utf16(bytes: &[u8]) -> String {
    let u16 = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .take_while(|&c| c != 0)
        .collect::<Vec<u16>>();

    String::from_utf16_lossy(&u16)
}

pub fn wrap(s: &str, width: usize) -> String {
    let (out, ..) = s.split_whitespace().fold(
        (String::with_capacity(s.len()), 0),
        |(mut out, mut line_len), word| {
            if line_len + word.len() > width && line_len > 0 {
                out.push('\n');
                line_len = 0;
            } else if line_len > 0 {
                out.push(' ');
                line_len += 1;
            }

            out.push_str(word);
            line_len += word.len();

            (out, line_len)
        },
    );

    out
}

pub fn ellipsis(s: &str, width: usize) -> String {
    match s.char_indices().nth(width) {
        Some((i, _)) => format!("{}...", &s[..i]),
        None => s.into(),
    }
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

    #[test]
    fn wrap() {
        let line = "the quick brown fox jumped over the lazy dog";
        let result = super::wrap(line, 20);
        assert_eq!(result, "the quick brown fox\njumped over the lazy\ndog");
    }

    #[test]
    fn ellipsis() {
        let line = "Foo Bar Baz";
        let result = super::ellipsis(line, 5);
        assert_eq!(result, "Foo B...");
    }
}
