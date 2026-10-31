use std::fmt;
use std::str::FromStr;

/// Crockford base32: no I, L, O or U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Crockford's check symbols: the alphabet, then `* ~ $ = U` for 32–36.
const CHECK: &[u8; 37] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ*~$=U";

/// The platform health id (identifiers.md): 11 random Crockford base32 characters and a
/// Crockford mod-37 check character, minted only at central when a golden record is created,
/// and shown in groups of four (`ABCD-EFGH-JKM*`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ecid(String);

/// Not an ECID: wrong length, a character outside the alphabet, or a bad check character.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a valid ECID")]
pub struct InvalidEcid;

fn check(body: &[u8]) -> u8 {
    let value = body.iter().fold(0u64, |acc, c| {
        let digit = ALPHABET.iter().position(|a| a == c).unwrap_or(0) as u64;
        acc * 32 + digit
    });
    CHECK[usize::try_from(value % 37).unwrap_or(0)]
}

impl Ecid {
    /// An ECID from 55 random bits (the low 55 of `random`); the caller supplies the randomness.
    pub fn from_random(random: u64) -> Self {
        let mut body = [0u8; 11];
        for (i, slot) in body.iter_mut().enumerate() {
            let shift = 5 * (10 - i);
            *slot = ALPHABET[usize::try_from((random >> shift) & 31).unwrap_or(0)];
        }
        let mut text = String::from_utf8_lossy(&body).into_owned();
        text.push(char::from(check(&body)));
        Self(text)
    }

    /// The 12 characters, without grouping.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Grouped for display: `XXXX-XXXX-XXXX`.
    pub fn grouped(&self) -> String {
        format!("{}-{}-{}", &self.0[..4], &self.0[4..8], &self.0[8..])
    }
}

impl fmt::Display for Ecid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Ecid {
    type Err = InvalidEcid;

    /// Accepts the grouped or plain form, in either case; Crockford's `I`/`L` read as `1` and
    /// `O` as `0`.
    fn from_str(text: &str) -> Result<Self, InvalidEcid> {
        let normal: Vec<u8> = text
            .bytes()
            .filter(|b| *b != b'-')
            .map(|b| match b.to_ascii_uppercase() {
                b'I' | b'L' => b'1',
                b'O' => b'0',
                other => other,
            })
            .collect();
        let [body @ .., last] = normal.as_slice() else {
            return Err(InvalidEcid);
        };
        if body.len() != 11 || !body.iter().all(|c| ALPHABET.contains(c)) || check(body) != *last {
            return Err(InvalidEcid);
        }
        Ok(Self(String::from_utf8_lossy(&normal).into_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_ecids_parse_and_typos_are_caught() {
        for random in [0u64, 1, 0x7f_ffff_ffff_ffff, 0x1234_5678_9abc_def0] {
            let ecid = Ecid::from_random(random);
            assert_eq!(ecid.as_str().len(), 12);
            assert_eq!(ecid.grouped().parse::<Ecid>(), Ok(ecid.clone()));
            assert_eq!(
                ecid.as_str().to_lowercase().parse::<Ecid>(),
                Ok(ecid.clone())
            );
            // One wrong character is caught by the check.
            let mut typo = ecid.as_str().as_bytes().to_vec();
            typo[3] = if typo[3] == b'7' { b'8' } else { b'7' };
            assert_eq!(
                String::from_utf8_lossy(&typo).parse::<Ecid>(),
                Err(InvalidEcid)
            );
        }
        assert_eq!("SHORT".parse::<Ecid>(), Err(InvalidEcid));
    }

    #[test]
    fn transposition_of_adjacent_characters_is_caught() {
        let ecid = Ecid::from_random(0x0aa5_5aa5_5aa5_5aa5);
        let mut swapped = ecid.as_str().as_bytes().to_vec();
        let i = (0..10).find(|&i| swapped[i] != swapped[i + 1]).unwrap_or(0);
        swapped.swap(i, i + 1);
        assert_eq!(
            String::from_utf8_lossy(&swapped).parse::<Ecid>(),
            Err(InvalidEcid)
        );
    }
}
