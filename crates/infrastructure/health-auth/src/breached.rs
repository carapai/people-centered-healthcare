//! The breached-password list (offline-auth.md § Credentials; #189): SecLists'
//! `xato-net-10-million-passwords-1000000` (MIT), shipped in the binary as a Bloom filter, so a
//! new password is checked the same way at central and on an edge with no network.
//!
//! **The filter format:**
//! - the magic `HBF1`;
//! - `m` (bits, u64 little-endian), `k` (hashes, u32) and `n` (entries, u64);
//! - the bit array.
//!
//! **Lookup:** bit `i` of `k` is `(h1 + i·h2) mod m`, where `h1` and `h2` are the first two
//! little-endian u64s of the password's SHA-256 (`h2` forced odd). The filter answers "maybe
//! listed" for about 0.1% of other passwords, never "not listed" for a listed one.

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 4] = b"HBF1";

/// The platform's list, built by `examples/breached_filter.rs` (see `data/README.md`).
static PLATFORM: &[u8] = include_bytes!("../data/breached-passwords.bloom");

/// The bytes are not a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a breached-password filter")]
pub struct InvalidFilter;

/// A set of breached passwords.
#[derive(Clone, PartialEq, Eq)]
pub struct BreachedPasswords {
    m: u64,
    k: u32,
    n: u64,
    bits: Vec<u8>,
}

impl std::fmt::Debug for BreachedPasswords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BreachedPasswords")
            .field("entries", &self.n)
            .field("bits", &self.m)
            .field("hashes", &self.k)
            .finish()
    }
}

fn hashes(password: &str) -> (u64, u64) {
    let digest = Sha256::digest(password.as_bytes());
    let word = |at: usize| {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&digest[at..at + 8]);
        u64::from_le_bytes(bytes)
    };
    (word(0), word(8) | 1)
}

impl BreachedPasswords {
    /// The platform's list, parsed once.
    pub fn platform() -> &'static Self {
        static PARSED: OnceLock<BreachedPasswords> = OnceLock::new();
        PARSED.get_or_init(|| {
            Self::from_bytes(PLATFORM)
                .unwrap_or_else(|_| unreachable!("the shipped filter is valid"))
        })
    }

    /// A filter holding `passwords`, sized for `false_positive` (e.g. 0.001).
    pub fn build<'a>(
        passwords: impl ExactSizeIterator<Item = &'a str>,
        false_positive: f64,
    ) -> Self {
        let n = passwords.len().max(1) as f64;
        let m = (-(n * false_positive.ln()) / std::f64::consts::LN_2.powi(2))
            .ceil()
            .max(64.0);
        let k = ((m / n) * std::f64::consts::LN_2).round().clamp(1.0, 32.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (m, k) = (m as u64, k as u32);
        let mut filter = Self {
            m,
            k,
            n: 0,
            bits: vec![0; usize::try_from(m.div_ceil(8)).unwrap_or(usize::MAX)],
        };
        for password in passwords {
            filter.insert(password);
        }
        filter
    }

    fn positions(&self, password: &str) -> impl Iterator<Item = u64> + '_ {
        let (h1, h2) = hashes(password);
        (0..u64::from(self.k)).map(move |i| h1.wrapping_add(i.wrapping_mul(h2)) % self.m)
    }

    fn insert(&mut self, password: &str) {
        let positions: Vec<u64> = self.positions(password).collect();
        for bit in positions {
            if let Some(byte) = usize::try_from(bit / 8)
                .ok()
                .and_then(|b| self.bits.get_mut(b))
            {
                *byte |= 1 << (bit % 8);
            }
        }
        self.n += 1;
    }

    /// Whether `password` is (probably) on the list.
    pub fn contains(&self, password: &str) -> bool {
        self.positions(password).all(|bit| {
            usize::try_from(bit / 8)
                .ok()
                .and_then(|b| self.bits.get(b))
                .is_some_and(|byte| byte & (1 << (bit % 8)) != 0)
        })
    }

    /// How many passwords the filter holds.
    pub fn len(&self) -> u64 {
        self.n
    }

    /// Whether the filter holds none.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The filter in its file format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(24 + self.bits.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.m.to_le_bytes());
        out.extend_from_slice(&self.k.to_le_bytes());
        out.extend_from_slice(&self.n.to_le_bytes());
        out.extend_from_slice(&self.bits);
        out
    }

    /// A filter from its file format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InvalidFilter> {
        let header = bytes.get(..24).ok_or(InvalidFilter)?;
        if &header[..4] != MAGIC {
            return Err(InvalidFilter);
        }
        let u64_at = |at: usize| {
            let mut word = [0u8; 8];
            word.copy_from_slice(&header[at..at + 8]);
            u64::from_le_bytes(word)
        };
        let mut k = [0u8; 4];
        k.copy_from_slice(&header[12..16]);
        let (m, k, n) = (u64_at(4), u32::from_le_bytes(k), u64_at(16));
        let bits = bytes[24..].to_vec();
        if m == 0 || k == 0 || u64::try_from(bits.len()).ok() != Some(m.div_ceil(8)) {
            return Err(InvalidFilter);
        }
        Ok(Self { m, k, n, bits })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_round_trips_and_never_misses_a_member() {
        let members = ["123456", "password", "Kampala2024", "correct horse battery"];
        let filter = BreachedPasswords::build(members.iter().copied(), 0.001);
        let reread = BreachedPasswords::from_bytes(&filter.to_bytes()).expect("filter");
        assert_eq!(reread, filter);
        assert!(members.iter().all(|m| reread.contains(m)));
        assert_eq!(reread.len(), 4);
        assert_eq!(
            BreachedPasswords::from_bytes(b"HBF1short"),
            Err(InvalidFilter)
        );
    }

    #[test]
    fn the_platform_list_holds_common_passwords_and_few_others() {
        let list = BreachedPasswords::platform();
        // 1,000,000 lines, 999,996 distinct.
        assert_eq!(list.len(), 999_996);
        for common in ["123456", "password", "qwerty", "iloveyou", "football"] {
            assert!(list.contains(common), "{common}");
        }
        // Random strong passphrases: false positives stay near the configured 0.1%.
        let hits = (0..10_000)
            .filter(|i| list.contains(&format!("tumbling-otter-{i}-quarry-{}", i * 7919)))
            .count();
        assert!(hits < 30, "{hits} false positives in 10,000");
    }
}
