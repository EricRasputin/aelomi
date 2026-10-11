//! `format.json`, the data folder's format version.

/// The data folder format this build reads and writes. Migrations are additive and bump it.
pub(crate) const FORMAT_VERSION: u32 = 1;

pub(crate) fn encode_format() -> String {
    format!("{{\n  \"format\": {FORMAT_VERSION}\n}}\n")
}

/// Reads `{"format": <n>}`, ignoring whitespace. Anything else is `None`.
pub(crate) fn parse_format(bytes: &[u8]) -> Option<u32> {
    let text = std::str::from_utf8(bytes).ok()?;
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = compact.strip_prefix("{\"format\":")?.strip_suffix('}')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_json_round_trips() {
        assert_eq!(encode_format(), "{\n  \"format\": 1\n}\n");
        assert_eq!(parse_format(encode_format().as_bytes()), Some(1));
        assert_eq!(parse_format(b"{\"format\":2}"), Some(2));
        assert_eq!(parse_format(b" { \"format\" : 12 } "), Some(12));
        for bad in [
            &b""[..],
            b"{}",
            b"{\"format\":\"1\"}",
            b"{\"format\":-1}",
            b"{\"format\":1,\"x\":2}",
            b"\xff",
        ] {
            assert_eq!(
                parse_format(bad),
                None,
                "{:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }
}
