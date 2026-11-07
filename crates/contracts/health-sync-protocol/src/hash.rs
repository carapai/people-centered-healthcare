use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A sha-256 digest, written as 64 lowercase hex digits.
///
/// For an item it is the hash of the body's canonical JSON ([`ContentHash::of`]): object keys
/// sorted, no whitespace, strings and numbers as `serde_json` writes them. It detects an `op_id`
/// reused for different content, and a duplicate that changes nothing. For a blob it is the hash
/// of its bytes ([`ContentHash::of_bytes`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash([u8; 32]);

/// Text that is not 64 hex digits.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a sha-256 hex digest")]
pub struct ContentHashError(pub String);

impl ContentHash {
    /// The hash of `body`'s canonical JSON.
    pub fn of(body: &Value) -> Self {
        let mut canonical = String::new();
        write_canonical(body, &mut canonical);
        Self::of_bytes(canonical.as_bytes())
    }

    /// The hash of `bytes`.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut digest = [0; 32];
        digest.copy_from_slice(&Sha256::digest(bytes));
        Self(digest)
    }
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            out.push('{');
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(value, out);
            }
            out.push('}');
        }
        Value::Array(values) => {
            out.push('[');
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(value, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&scalar.to_string()),
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl FromStr for ContentHash {
    type Err = ContentHashError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ContentHashError(text.to_owned());
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(invalid());
        }
        let mut digest = [0; 32];
        for (byte, pair) in digest.iter_mut().zip(bytes.chunks(2)) {
            let pair = std::str::from_utf8(pair).map_err(|_| invalid())?;
            if pair.bytes().any(|b| b.is_ascii_uppercase()) {
                return Err(invalid());
            }
            *byte = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
        }
        Ok(Self(digest))
    }
}

impl TryFrom<String> for ContentHash {
    type Error = ContentHashError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.to_string()
    }
}
