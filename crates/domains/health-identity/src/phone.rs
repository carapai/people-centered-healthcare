use std::fmt;

use serde::{Deserialize, Serialize};

/// A phone number in E.164 form, e.g. `+256772123456`: a patient's Login Account is one per
/// verified phone number (ADR 0029).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PhoneNumber(String);

/// Why a phone number was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPhone {
    /// Something other than digits, spaces, `-`, `.`, `(`, `)` and a leading `+`.
    #[error("a phone number has only digits")]
    NotDigits,
    /// E.164 numbers have 8 to 15 digits, and the country code never starts with 0.
    #[error("not a valid international phone number")]
    Length,
}

impl PhoneNumber {
    /// Parses `input` as written by a person: `+256 772 123 456`, `00256772123456`, or the
    /// national form `0772 123456`, which takes `country_code` (e.g. `256`).
    pub fn parse(input: &str, country_code: &str) -> Result<Self, InvalidPhone> {
        let trimmed = input.trim();
        let (international, rest) = match trimmed.strip_prefix('+') {
            Some(rest) => (true, rest),
            None => (false, trimmed),
        };
        let mut digits = String::with_capacity(rest.len());
        for c in rest.chars() {
            match c {
                '0'..='9' => digits.push(c),
                ' ' | '-' | '.' | '(' | ')' => {}
                _ => return Err(InvalidPhone::NotDigits),
            }
        }
        let digits = if international {
            digits
        } else if let Some(rest) = digits.strip_prefix("00") {
            rest.to_owned()
        } else if let Some(rest) = digits.strip_prefix('0') {
            format!("{country_code}{rest}")
        } else {
            digits
        };
        if !(8..=15).contains(&digits.len()) || digits.starts_with('0') {
            return Err(InvalidPhone::Length);
        }
        Ok(Self(format!("+{digits}")))
    }

    /// The E.164 form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PhoneNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PhoneNumber {
    type Error = InvalidPhone;

    /// Only the E.164 form, as stored.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.starts_with('+') {
            return Err(InvalidPhone::Length);
        }
        let parsed = Self::parse(&value, "")?;
        if parsed.0 == value {
            Ok(parsed)
        } else {
            Err(InvalidPhone::NotDigits)
        }
    }
}

impl From<PhoneNumber> for String {
    fn from(phone: PhoneNumber) -> Self {
        phone.0
    }
}
