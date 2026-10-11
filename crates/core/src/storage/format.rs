pub(crate) const FORMAT_VERSION: u64 = 1;

pub(crate) fn encode_format() -> String {
    format!("{{\n  \"format\": {FORMAT_VERSION}\n}}\n")
}

pub(crate) fn parse_format(bytes: &[u8]) -> Option<u64> {
    let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    json.get("format")?.as_u64()
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
            parse_format(b"{\"format\":4294967296}"),
            Some(4_294_967_296)
        );
        assert_eq!(
            parse_format(b"{\"format\":18446744073709551615}"),
            Some(u64::MAX)
        );
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
