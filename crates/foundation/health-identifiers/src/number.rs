use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Why a code, format or sequence step is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NumberError {
    /// A facility, node or prefix code outside `[A-Z0-9]{1,8}`.
    #[error("`{0}` must be 1–8 characters of A–Z and 0–9")]
    InvalidCode(String),
    /// A yearly-reset counter was asked for a year earlier than the one it last minted in.
    /// Minting would reuse numbers, so it is refused.
    #[error("counter last minted in {last}, asked to mint in {asked}; numbers are never reused")]
    YearWentBack {
        /// The year of the last minted number.
        last: i32,
        /// The year asked for.
        asked: i32,
    },
    /// The counter cannot advance further.
    #[error("sequence exhausted")]
    Exhausted,
}

/// A facility code, node short code or number prefix: 1–8 characters of `A–Z` and `0–9`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ShortCode(String);

impl ShortCode {
    /// The code as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ShortCode {
    type Err = NumberError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let valid = (1..=8).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(NumberError::InvalidCode(text.to_owned()))
        }
    }
}

impl TryFrom<String> for ShortCode {
    type Error = NumberError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<ShortCode> for String {
    fn from(code: ShortCode) -> Self {
        code.0
    }
}

impl fmt::Display for ShortCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An optional check character appended to a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckDigit {
    /// Luhn mod N over `0–9A–Z` (N = 36), computed over every character except hyphens.
    #[serde(rename = "luhn-mod-36")]
    LuhnMod36,
}

/// The format of one identifier type, a Published Definition in the country package (ADR 0014).
///
/// The number is `[{prefix}-]{facility}-{node}[-{year}]-{sequence}[check]`, with the sequence
/// zero-padded to `padding` digits. The default is `{facility}-{node}-{sequence}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NumberFormat {
    /// A leading segment naming the identifier type, e.g. `MRN`.
    #[serde(default)]
    pub prefix: Option<ShortCode>,
    /// Minimum digits of the sequence segment; longer sequences are never truncated.
    #[serde(default)]
    pub padding: u8,
    /// An optional check character.
    #[serde(default)]
    pub check_digit: Option<CheckDigit>,
    /// Restart the sequence each calendar year, with a year segment in the number.
    #[serde(default)]
    pub yearly_reset: bool,
}

impl NumberFormat {
    /// Formats sequence value `sequence`, minted in `year` by `node` at `facility`.
    pub fn format(
        &self,
        facility: &ShortCode,
        node: &ShortCode,
        year: i32,
        sequence: u64,
    ) -> String {
        let mut segments: Vec<String> = Vec::with_capacity(5);
        if let Some(prefix) = &self.prefix {
            segments.push(prefix.to_string());
        }
        segments.push(facility.to_string());
        segments.push(node.to_string());
        if self.yearly_reset {
            segments.push(year.to_string());
        }
        segments.push(format!(
            "{sequence:0width$}",
            width = usize::from(self.padding)
        ));
        let mut number = segments.join("-");
        if let Some(CheckDigit::LuhnMod36) = self.check_digit {
            let body: String = number.chars().filter(|c| *c != '-').collect();
            number.push(luhn_mod_36(&body));
        }
        number
    }
}

const ALPHABET: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// The Luhn mod N check character for `body` over `0–9A–Z`. Characters outside the alphabet are
/// skipped; callers pass only validated codes and digits.
pub fn luhn_mod_36(body: &str) -> char {
    const N: u32 = 36;
    let mut factor = 2;
    let mut sum = 0;
    for byte in body.bytes().rev() {
        let Some(code) = ALPHABET.iter().position(|a| *a == byte) else {
            continue;
        };
        let addend = factor * u32::try_from(code).unwrap_or(0);
        factor = if factor == 2 { 1 } else { 2 };
        sum += addend / N + addend % N;
    }
    let check = (N - sum % N) % N;
    char::from(ALPHABET[usize::try_from(check).unwrap_or(0)])
}

/// The persisted state of one node-local counter. The store increments it in the same
/// transaction as the record that takes the number; this type holds the rules for that step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequenceState {
    /// The year of the last minted number, for yearly-reset formats.
    pub year: Option<i32>,
    /// The last minted sequence value; 0 when nothing has been minted.
    pub last: u64,
}

impl SequenceState {
    /// The next state and the sequence value to mint in `year`. Values start at 1 and never
    /// repeat: a yearly reset restarts at 1 only in a later year, and an earlier year is refused.
    pub fn next(self, format: &NumberFormat, year: i32) -> Result<(Self, u64), NumberError> {
        if format.yearly_reset {
            match self.year {
                Some(last) if last > year => {
                    return Err(NumberError::YearWentBack { last, asked: year });
                }
                Some(last) if last == year => {}
                _ => {
                    return Ok((
                        Self {
                            year: Some(year),
                            last: 1,
                        },
                        1,
                    ));
                }
            }
        }
        let value = self.last.checked_add(1).ok_or(NumberError::Exhausted)?;
        Ok((
            Self {
                year: self.year.filter(|_| format.yearly_reset),
                last: value,
            },
            value,
        ))
    }
}
