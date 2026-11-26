//! Pure, host-testable logic for the ELAM driver.
//!
//! Everything here is `no_std` (with `alloc`) so the driver can depend on it,
//! but the unit tests run on a normal host toolchain.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt::{self, Write};

/// Length of a SHA-256 digest in bytes.
pub const SHA256_LEN: usize = 32;

/// `BCRYPT_SHA256_ALGORITHM`, as reported in `BDCB_IMAGE_INFORMATION.ImageHashAlgorithm`.
pub const BCRYPT_SHA256_ALGORITHM: u32 = 0x0000_800c;

/// Result of classifying a boot-start image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    KnownGood,
    KnownBad,
    Unknown,
}

/// Classify an image purely by its hash.
///
/// Only SHA-256 digests are matched. Anything with a different algorithm or
/// length yields [`Classification::Unknown`], which keeps the boot loader's
/// default policy in effect.
pub fn classify_by_hash(
    algorithm: u32,
    hash: &[u8],
    known_good: &[[u8; SHA256_LEN]],
    known_bad: &[[u8; SHA256_LEN]],
) -> Classification {
    if algorithm != BCRYPT_SHA256_ALGORITHM || hash.len() != SHA256_LEN {
        return Classification::Unknown;
    }

    if known_bad.iter().any(|h| h.as_slice() == hash) {
        Classification::KnownBad
    } else if known_good.iter().any(|h| h.as_slice() == hash) {
        Classification::KnownGood
    } else {
        Classification::Unknown
    }
}

/// Decode a UTF-16 (wide) buffer into a `String`, lossily.
///
/// A null or empty buffer yields `None`. The caller is responsible for turning
/// the raw `UNICODE_STRING` fields into a slice only when they are valid.
pub fn decode_utf16_lossy(wide: Option<&[u16]>) -> Option<String> {
    match wide {
        Some(w) if !w.is_empty() => Some(String::from_utf16_lossy(w)),
        _ => None,
    }
}

/// Render `data` as lowercase hex, `bytes_per_line` bytes per line, separated by
/// CRLF. A trailing space follows every byte.
pub fn format_hex(data: &[u8], bytes_per_line: usize) -> String {
    let per_line = bytes_per_line.max(1);
    let mut out = String::with_capacity(data.len() * 3 + data.len() / per_line * 2);

    for (i, byte) in data.iter().enumerate() {
        if i != 0 && i % per_line == 0 {
            out.push_str("\r\n");
        }
        // Writing into a String cannot fail.
        let _ = write!(out, "{byte:02x} ");
    }

    out
}

/// Error returned by [`parse_hash_lines`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashParseError {
    /// 1-based line number in the input text.
    pub line: usize,
    /// The offending line (trimmed).
    pub input: String,
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid SHA-256 digest (expected 64 hex chars): {:?}", self.input)
    }
}

/// Parse a hash allowlist: one SHA-256 digest per line as 64 hex characters.
///
/// Blank lines and `#` comments are ignored. Spaces, tabs, `,` and `:` may be
/// used as separators (e.g. `aa:bb:...`). The first malformed line is returned
/// as [`HashParseError`].
pub fn parse_hash_lines(text: &str) -> core::result::Result<Vec<[u8; SHA256_LEN]>, HashParseError> {
    let mut hashes = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let hex: String =
            line.chars().filter(|c| !c.is_whitespace() && *c != ',' && *c != ':').collect();

        match decode_sha256(&hex) {
            Some(hash) => hashes.push(hash),
            None => {
                return Err(HashParseError { line: index + 1, input: raw.trim().to_string() });
            },
        }
    }

    Ok(hashes)
}

fn decode_sha256(hex: &str) -> Option<[u8; SHA256_LEN]> {
    if hex.len() != SHA256_LEN * 2 {
        return None;
    }

    let mut out = [0u8; SHA256_LEN];
    let bytes = hex.as_bytes();
    for (i, out_byte) in out.iter_mut().enumerate() {
        let hi = (bytes[i * 2] as char).to_digit(16)?;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
        *out_byte = (hi * 16 + lo) as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: [u8; SHA256_LEN] = [0xAA; SHA256_LEN];
    const BAD: [u8; SHA256_LEN] = [0xBB; SHA256_LEN];

    fn classify(algorithm: u32, hash: &[u8]) -> Classification {
        classify_by_hash(algorithm, hash, &[GOOD], &[BAD])
    }

    #[test]
    fn matches_known_good() {
        assert_eq!(classify(BCRYPT_SHA256_ALGORITHM, &GOOD), Classification::KnownGood);
    }

    #[test]
    fn matches_known_bad() {
        assert_eq!(classify(BCRYPT_SHA256_ALGORITHM, &BAD), Classification::KnownBad);
    }

    #[test]
    fn bad_takes_precedence_over_good() {
        // If a hash somehow appears in both tables, treat it as malicious.
        assert_eq!(
            classify_by_hash(BCRYPT_SHA256_ALGORITHM, &BAD, &[BAD], &[BAD]),
            Classification::KnownBad
        );
    }

    #[test]
    fn unknown_hash_is_unknown() {
        assert_eq!(classify(BCRYPT_SHA256_ALGORITHM, &[0xCC; SHA256_LEN]), Classification::Unknown);
    }

    #[test]
    fn wrong_algorithm_is_unknown() {
        assert_eq!(classify(0x0000_800d, &GOOD), Classification::Unknown);
    }

    #[test]
    fn truncated_hash_is_unknown() {
        assert_eq!(classify(BCRYPT_SHA256_ALGORITHM, &GOOD[..16]), Classification::Unknown);
    }

    #[test]
    fn empty_hash_is_unknown() {
        assert_eq!(classify(BCRYPT_SHA256_ALGORITHM, &[]), Classification::Unknown);
    }

    #[test]
    fn decode_handles_none_empty_and_text() {
        assert_eq!(decode_utf16_lossy(None), None);
        assert_eq!(decode_utf16_lossy(Some(&[])), None);

        let wide: [u16; 5] = [b'\\' as u16, b'?' as u16, b'\\' as u16, b'C' as u16, b':' as u16];
        assert_eq!(decode_utf16_lossy(Some(&wide)).as_deref(), Some("\\?\\C:"));
    }

    #[test]
    fn hex_wraps_at_16_bytes() {
        let data: [u8; 17] = core::array::from_fn(|i| i as u8);
        let out = format_hex(&data, 16);

        assert!(out.starts_with("00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f \r\n"));
        assert!(out.ends_with("10 "));
        assert_eq!(out.matches("\r\n").count(), 1);
    }

    #[test]
    fn hex_of_empty_is_empty() {
        assert_eq!(format_hex(&[], 16), "");
    }

    #[test]
    fn hex_zero_line_width_does_not_divide_by_zero() {
        let out = format_hex(&[1, 2, 3], 0);
        assert_eq!(out.matches("\r\n").count(), 2);
    }

    // ---- hash allowlist parser ----

    const HASH_A: &str = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
    const HASH_A_BYTES: [u8; SHA256_LEN] = [
        0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
        0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99,
    ];

    #[test]
    fn parse_hash_lines_ignores_comments_and_separators() {
        let text = format!("# header comment\n{}:{}  # inline\n\n", &HASH_A[..32], &HASH_A[32..]);
        let hashes = parse_hash_lines(&text).unwrap();
        assert_eq!(hashes, vec![HASH_A_BYTES]);
    }

    #[test]
    fn parse_hash_lines_preserves_order() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let hashes = parse_hash_lines(&format!("{a}\n{b}\n")).unwrap();
        assert_eq!(hashes, vec![[0xaa; SHA256_LEN], [0xbb; SHA256_LEN]]);
    }

    #[test]
    fn parse_hash_lines_rejects_wrong_length() {
        let err = parse_hash_lines("abcd\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn parse_hash_lines_rejects_non_hex() {
        let err = parse_hash_lines(&"z".repeat(64)).unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.to_string().contains("invalid SHA-256"));
    }

    #[test]
    fn parse_hash_lines_reports_error_line() {
        let good = "a".repeat(64);
        let err = parse_hash_lines(&format!("{good}\n\nnot-hex\n")).unwrap_err();
        assert_eq!(err.line, 3);
    }

    #[test]
    fn parse_hash_lines_empty_is_empty() {
        assert!(parse_hash_lines("").unwrap().is_empty());
        assert!(parse_hash_lines("# only a comment\n\n").unwrap().is_empty());
    }

    #[test]
    fn parse_hash_lines_handles_crlf() {
        let good = "a".repeat(64);
        assert_eq!(
            parse_hash_lines(&format!("# c\r\n{good}\r\n")).unwrap(),
            vec![[0xaa; SHA256_LEN]]
        );
    }
}
