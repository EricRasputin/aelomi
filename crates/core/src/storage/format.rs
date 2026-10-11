pub(crate) const FORMAT_VERSION: u32 = 1;

pub(crate) fn encode_format() -> String {
    format!("{{\n  \"format\": {FORMAT_VERSION}\n}}\n")
}

/// Reads only `format`, so a later build can add keys and still be refused as newer.
pub(crate) fn parse_format(bytes: &[u8]) -> Option<u32> {
    let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    u32::try_from(json.get("format")?.as_u64()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_json_round_trips_and_reads_only_its_format() {
        assert_eq!(encode_format(), "{\n  \"format\": 1\n}\n");
        assert_eq!(parse_format(encode_format().as_bytes()), Some(1));
        assert_eq!(parse_format(b"{\"format\":2}"), Some(2));
        assert_eq!(parse_format(b" { \"format\" : 12 } "), Some(12));
        assert_eq!(parse_format(b"{\"format\":1,\"x\":2}"), Some(1));
        assert_eq!(
            parse_format(b"{\"x\":[1,{\"format\":3},\"}\"],\"format\":2}"),
            Some(2)
        );
        for bad in [
            &b""[..],
            b"{}",
            b"{\"x\":1}",
            b"{\"format\":\"1\"}",
            b"{\"format\":-1}",
            b"{\"format\":1.5}",
            b"{\"format\":null}",
            b"{\"format\":4294967296}",
            b"[{\"format\":1}]",
            b"{\"format\":1",
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
