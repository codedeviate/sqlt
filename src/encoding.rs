//! Input/output encoding for SQL data.
//!
//! Source SQL files don't have to be UTF-8 — at least one of the user's
//! production systems emits ISO-8859-1. The `--encoding` flag on each
//! subcommand selects how to decode incoming bytes (file or stdin) and how
//! to encode SQL output (`emit` and `translate`). JSON output (`parse`)
//! always stays UTF-8 because the JSON spec says so; the encoding flag
//! does not affect it on the output side.
//!
//! Decoding is strict: if the bytes contain sequences invalid for the
//! declared encoding, we error rather than silently substitute U+FFFD.
//! That's the right default — silent substitution turns a column value
//! like `'café'` into `'caf?'` and you don't notice until production.

use std::str::FromStr;

use crate::error::Error;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    /// UTF-8. Default. JSON output is always emitted as UTF-8 regardless of
    /// this setting.
    #[default]
    Utf8,
    /// ISO-8859-1, a.k.a. Latin-1. A pure 8-bit code page; every byte
    /// 0x00..=0xFF maps to a distinct code point, so decoding is infallible.
    Iso8859_1,
    /// Windows-1252, the superset of Latin-1 that Microsoft systems often
    /// label as "Latin1" or "ANSI". Adds printable characters (curly quotes,
    /// em dash, euro sign, etc.) in the 0x80..=0x9F range that ISO-8859-1
    /// leaves as control codes.
    Windows1252,
}

impl Encoding {
    fn rs(self) -> &'static encoding_rs::Encoding {
        match self {
            Encoding::Utf8 => encoding_rs::UTF_8,
            Encoding::Iso8859_1 => encoding_rs::WINDOWS_1252,
            // Note: encoding_rs follows the WHATWG mapping where
            // ISO-8859-1 and windows-1252 share the same decoder. That
            // matches what most "Latin1" tagged files actually contain in
            // practice (curly quotes etc. in 0x80..=0x9F).
            Encoding::Windows1252 => encoding_rs::WINDOWS_1252,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Iso8859_1 => "iso-8859-1",
            Encoding::Windows1252 => "windows-1252",
        }
    }

    /// Decode bytes into a `String`. Returns `Error::Encoding` if the input
    /// is not valid for the declared encoding (UTF-8 only — the 8-bit code
    /// pages cover every byte and never fail).
    pub fn decode(self, bytes: &[u8]) -> Result<String, Error> {
        let (cow, _enc, had_errors) = self.rs().decode(bytes);
        if had_errors {
            return Err(Error::Encoding(format!(
                "input is not valid {}",
                self.as_str()
            )));
        }
        Ok(cow.into_owned())
    }

    /// Encode a `String` into bytes for output. Returns `Error::Encoding` if
    /// the string contains code points the target encoding cannot represent.
    pub fn encode(self, s: &str) -> Result<Vec<u8>, Error> {
        let (cow, _enc, had_errors) = self.rs().encode(s);
        if had_errors {
            return Err(Error::Encoding(format!(
                "output contains characters that cannot be represented in {}",
                self.as_str()
            )));
        }
        Ok(cow.into_owned())
    }
}

impl FromStr for Encoding {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('_', "-").as_str() {
            "utf-8" | "utf8" => Ok(Encoding::Utf8),
            "iso-8859-1" | "latin1" | "latin-1" | "iso8859-1" => Ok(Encoding::Iso8859_1),
            "windows-1252" | "cp1252" | "win1252" => Ok(Encoding::Windows1252),
            other => Err(Error::UnknownEncoding(other.to_string())),
        }
    }
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin1_roundtrip() {
        let s = "café — naïve";
        let bytes = Encoding::Iso8859_1.encode(s).expect("encode");
        // ö-style bytes are single-byte in latin1.
        assert!(bytes.contains(&0xE9), "expected 0xE9 for é");
        let decoded = Encoding::Iso8859_1.decode(&bytes).expect("decode");
        assert_eq!(decoded, s);
    }

    #[test]
    fn utf8_decoder_rejects_latin1_bytes() {
        // 0xE9 alone is invalid UTF-8 but valid Latin-1.
        let bytes = b"caf\xE9";
        assert!(Encoding::Utf8.decode(bytes).is_err());
        assert_eq!(Encoding::Iso8859_1.decode(bytes).unwrap(), "café");
    }

    #[test]
    fn unrepresentable_char_errors_on_encode() {
        // U+1F600 (emoji) cannot be encoded in latin1.
        assert!(Encoding::Iso8859_1.encode("hello 😀").is_err());
    }

    #[test]
    fn parses_aliases() {
        assert_eq!("utf8".parse::<Encoding>().unwrap(), Encoding::Utf8);
        assert_eq!("UTF-8".parse::<Encoding>().unwrap(), Encoding::Utf8);
        assert_eq!("latin1".parse::<Encoding>().unwrap(), Encoding::Iso8859_1);
        assert_eq!(
            "ISO-8859-1".parse::<Encoding>().unwrap(),
            Encoding::Iso8859_1
        );
        assert_eq!("cp1252".parse::<Encoding>().unwrap(), Encoding::Windows1252);
    }
}
