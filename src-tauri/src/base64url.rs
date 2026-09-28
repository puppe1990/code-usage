//! base64url (RFC 4648 §5, no padding) used by the Codex CLI: it encodes account keys into the
//! file names under `accounts/` and keeps the login e-mail inside the `id_token` payload.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encodes `bytes` as base64url without padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let mut buffer = [0u8; 3];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let bits = u32::from(buffer[0]) << 16 | u32::from(buffer[1]) << 8 | u32::from(buffer[2]);

        for shift in [18, 12, 6, 0].iter().take(chunk.len() + 1) {
            out.push(ALPHABET[(bits >> shift & 0x3f) as usize] as char);
        }
    }

    out
}

/// Decodes base64url text, with or without padding. `None` when it is not valid base64url.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut bits = 0u32;
    let mut count = 0;

    for byte in text.bytes() {
        if byte == b'=' || byte.is_ascii_whitespace() {
            continue;
        }
        let value = ALPHABET.iter().position(|&letter| letter == byte)? as u32;

        bits = bits << 6 | value;
        count += 1;
        if count == 4 {
            out.extend_from_slice(&bits.to_be_bytes()[1..]);
            bits = 0;
            count = 0;
        }
    }

    match count {
        0 => {}
        // one leftover character carries 6 bits, not enough for a byte
        1 => return None,
        leftover => {
            let padded = (bits << (6 * (4 - leftover))).to_be_bytes();
            out.extend_from_slice(&padded[1..leftover]);
        }
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file name `codex-auth` writes for this account key.
    const ACCOUNT_KEY: &str = "user-kJcQbqUPq7yMES0ALoCeVu1G::ccda98f9-75f5-47a4-b092-e84434db293f";
    const ACCOUNT_FILE: &str = "dXNlci1rSmNRYnFVUHE3eU1FUzBBTG9DZVZ1MUc6OmNjZGE5OGY5LTc1ZjUtNDdhNC1iMDkyLWU4NDQzNGRiMjkzZg";

    #[test]
    fn encodes_the_account_file_names_the_cli_writes() {
        assert_eq!(encode(ACCOUNT_KEY.as_bytes()), ACCOUNT_FILE);
        assert_eq!(decode(ACCOUNT_FILE).unwrap(), ACCOUNT_KEY.as_bytes());
    }

    #[test]
    fn hides_the_url_specific_characters() {
        assert_eq!(encode(&[0xfb, 0xff, 0xbf]), "-_-_");
    }

    #[test]
    fn decodes_padding_and_whitespace() {
        assert_eq!(decode("aGk=").unwrap(), b"hi");
        assert_eq!(decode("aGk").unwrap(), b"hi");
        assert_eq!(decode("aG k\n").unwrap(), b"hi");
    }

    #[test]
    fn refuses_text_that_is_not_base64url() {
        assert_eq!(decode("a"), None, "a single character holds no byte");
        assert_eq!(decode("aGk!"), None);
        assert_eq!(
            decode("not/a/path"),
            None,
            "`/` belongs to the standard alphabet"
        );
        assert_eq!(decode("ab.cd"), None);
    }
}
