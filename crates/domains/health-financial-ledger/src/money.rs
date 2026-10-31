//! Exact money (ADR 0030): `rust_decimal` amounts at their currency's ISO 4217 scale, rounded
//! half-even, and largest-remainder splits.

use std::fmt;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

/// An ISO 4217 currency this platform handles, with its minor-unit scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency {
    code: [u8; 3],
    scale: u32,
}

/// ISO 4217 codes and their minor units, for the currencies of the region and of donor funding.
const CURRENCIES: &[(&str, u32)] = &[
    ("BIF", 0),
    ("CDF", 2),
    ("ETB", 2),
    ("EUR", 2),
    ("GBP", 2),
    ("KES", 2),
    ("RWF", 0),
    ("SSP", 2),
    ("TZS", 2),
    ("UGX", 0),
    ("USD", 2),
];

/// A currency code the platform does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a supported ISO 4217 currency")]
pub struct UnknownCurrency(pub String);

impl Currency {
    /// The currency with ISO 4217 `code`.
    pub fn from_code(code: &str) -> Result<Self, UnknownCurrency> {
        CURRENCIES
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(c, scale)| {
                let bytes = c.as_bytes();
                Self {
                    code: [bytes[0], bytes[1], bytes[2]],
                    scale: *scale,
                }
            })
            .ok_or_else(|| UnknownCurrency(code.to_owned()))
    }

    /// The ISO 4217 code.
    pub fn code(&self) -> &str {
        std::str::from_utf8(&self.code).unwrap_or("???")
    }

    /// Digits after the decimal point (UGX: 0; USD: 2).
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// The smallest amount: one minor unit.
    pub fn minor_unit(&self) -> Decimal {
        Decimal::new(1, self.scale)
    }
}

impl TryFrom<String> for Currency {
    type Error = UnknownCurrency;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        Self::from_code(&code)
    }
}

impl From<Currency> for String {
    fn from(currency: Currency) -> Self {
        currency.code().to_owned()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// An amount in a currency, always at the currency's scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Money {
    amount: Decimal,
    currency: Currency,
}

/// An amount does not fit its currency.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    /// More decimal places than the currency has; round explicitly with [`Money::rounded`].
    #[error("{amount} has more decimal places than {currency} allows")]
    TooPrecise {
        /// The amount.
        amount: Decimal,
        /// The currency.
        currency: Currency,
    },
    /// Amounts in two currencies were combined.
    #[error("{0} and {1} cannot be combined")]
    CurrencyMismatch(Currency, Currency),
    /// A split needs at least one positive weight, and no negative ones.
    #[error("a split needs non-negative weights, at least one positive")]
    BadWeights,
}

impl Money {
    /// `amount` in `currency`, which must already be at the currency's scale.
    pub fn new(amount: Decimal, currency: Currency) -> Result<Self, MoneyError> {
        if amount.normalize().scale() > currency.scale() {
            return Err(MoneyError::TooPrecise { amount, currency });
        }
        Ok(Self {
            amount: amount.round_dp(currency.scale()),
            currency,
        })
    }

    /// `amount` rounded half-even to the currency's scale (ADR 0030).
    pub fn rounded(amount: Decimal, currency: Currency) -> Self {
        Self {
            amount: amount
                .round_dp_with_strategy(currency.scale(), RoundingStrategy::MidpointNearestEven),
            currency,
        }
    }

    /// Zero in `currency`.
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    /// The amount.
    pub fn amount(&self) -> Decimal {
        self.amount
    }

    /// The currency.
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Whether it is zero.
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// The sum, in the same currency.
    pub fn plus(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(self.currency, other.currency));
        }
        Ok(Self {
            amount: self.amount + other.amount,
            currency: self.currency,
        })
    }

    /// The difference, in the same currency.
    pub fn minus(self, other: Self) -> Result<Self, MoneyError> {
        self.plus(Self {
            amount: -other.amount,
            currency: other.currency,
        })
    }

    /// Splits this amount by `weights` with the largest-remainder method: each share is rounded
    /// down to a minor unit, then the minor units left over go one each to the largest
    /// remainders (ties to the earliest). The shares always sum to the whole amount exactly.
    pub fn split(&self, weights: &[Decimal]) -> Result<Vec<Self>, MoneyError> {
        if weights.iter().any(|w| w.is_sign_negative()) || weights.iter().all(Decimal::is_zero) {
            return Err(MoneyError::BadWeights);
        }
        if self.amount.is_sign_negative() {
            let shares = Self {
                amount: -self.amount,
                currency: self.currency,
            }
            .split(weights)?;
            return Ok(shares
                .into_iter()
                .map(|s| Self {
                    amount: -s.amount,
                    currency: s.currency,
                })
                .collect());
        }
        let total: Decimal = weights.iter().sum();
        let unit = self.currency.minor_unit();
        let units = (self.amount / unit).trunc();
        let mut floors = Vec::with_capacity(weights.len());
        let mut remainders = Vec::with_capacity(weights.len());
        for (index, weight) in weights.iter().enumerate() {
            let exact = units * *weight / total;
            let floor = exact.floor();
            floors.push(floor);
            remainders.push((exact - floor, index));
        }
        let mut left = units - floors.iter().sum::<Decimal>();
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, index) in remainders {
            if left <= Decimal::ZERO {
                break;
            }
            floors[index] += Decimal::ONE;
            left -= Decimal::ONE;
        }
        Ok(floors
            .into_iter()
            .map(|f| Self {
                amount: (f * unit).round_dp(self.currency.scale),
                currency: self.currency,
            })
            .collect())
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}
