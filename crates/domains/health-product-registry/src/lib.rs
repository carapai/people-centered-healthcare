//! The product master (resource-ownership.md: product-registry owns product identity, never
//! quantities; inventory-and-procurement.md § Records). Its authority is central.
//!
//! - **[`Product`]**: a stock product. It has its code (the Stock Unit's `product`), name,
//!   GTIN, base unit and pack sizes, the medicine codes it fills, and other names it goes by.
//!   [`Product::validate`] checks it, including the GTIN's check digit.
//! - **[`resolve`]**: the stock product a prescribed [`Medicine`] is dispensed as.
//!   - A coded medicine matches by its coding first.
//!   - Otherwise it matches by name or synonym, with case, spacing and punctuation ignored, so
//!     "Artemether / Lumefantrine 20/120mg" finds "artemether/lumefantrine 20/120 mg".
//!   - Two products that fit equally are [`Resolution::Ambiguous`], for the pharmacist to choose
//!     from. Inactive products are never chosen.

use serde::{Deserialize, Serialize};

/// A code from a code system.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Coding {
    /// The code system.
    pub system: String,
    /// The code.
    pub code: String,
}

/// A stock product.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    /// Its code, the Stock Unit's `product` (e.g. `amoxicillin-500`).
    pub code: String,
    /// What a person reads.
    pub name: String,
    /// Its GS1 GTIN, if it has one.
    pub gtin: Option<String>,
    /// The unit stock is counted in (tablet, capsule, ml).
    pub base_unit: String,
    /// The pack sizes it comes in, in base units.
    pub pack_sizes: Vec<u32>,
    /// The medicine codes it fills.
    pub medicines: Vec<Coding>,
    /// Other names it is prescribed by.
    pub synonyms: Vec<String>,
    /// Whether it may be chosen.
    pub active: bool,
}

/// Why a product is not valid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProductRefused {
    /// The code is not a lowercase slug (letters, digits and `-`).
    #[error("a product code is lowercase letters, digits and dashes")]
    Code,
    /// A required field is empty.
    #[error("a product needs its {0}")]
    Missing(&'static str),
    /// A pack size is zero.
    #[error("pack sizes are counted in base units, at least one")]
    PackSize,
    /// The GTIN is not 8, 12, 13 or 14 digits with a valid check digit.
    #[error("{0} is not a valid GTIN")]
    Gtin(String),
}

/// Whether `gtin` is a GTIN-8, -12, -13 or -14 with a valid GS1 check digit.
#[must_use]
pub fn valid_gtin(gtin: &str) -> bool {
    let digits: Vec<u32> = gtin.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() != gtin.len() || ![8, 12, 13, 14].contains(&digits.len()) {
        return false;
    }
    let (body, check) = digits.split_at(digits.len() - 1);
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d })
        .sum();
    (10 - sum % 10) % 10 == check[0]
}

impl Product {
    /// Checks the product.
    pub fn validate(&self) -> Result<(), ProductRefused> {
        let slug = !self.code.is_empty()
            && self
                .code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !slug {
            return Err(ProductRefused::Code);
        }
        if self.name.trim().is_empty() {
            return Err(ProductRefused::Missing("name"));
        }
        if self.base_unit.trim().is_empty() {
            return Err(ProductRefused::Missing("base unit"));
        }
        if self.pack_sizes.contains(&0) {
            return Err(ProductRefused::PackSize);
        }
        if let Some(gtin) = &self.gtin
            && !valid_gtin(gtin)
        {
            return Err(ProductRefused::Gtin(gtin.clone()));
        }
        Ok(())
    }
}

/// A medicine as prescribed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Medicine {
    /// What was written.
    pub text: String,
    /// Its coding, if it has one.
    pub coding: Option<Coding>,
}

/// What a medicine resolves to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Resolution {
    /// One product, by the medicine's code.
    ByCode {
        /// The product's code.
        product: String,
    },
    /// One product, by name or synonym.
    ByName {
        /// The product's code.
        product: String,
    },
    /// Several fit equally: the pharmacist chooses.
    Ambiguous {
        /// Their codes, in order.
        products: Vec<String>,
    },
    /// None fits.
    Unknown,
}

impl Resolution {
    /// The one product, if there is one.
    #[must_use]
    pub fn product(&self) -> Option<&str> {
        match self {
            Self::ByCode { product } | Self::ByName { product } => Some(product),
            Self::Ambiguous { .. } | Self::Unknown => None,
        }
    }
}

/// A name with case, spacing and punctuation (other than `/` and `.`) ignored, and a number's
/// unit joined to it: "20/120 mg" and "20/120mg" are the same.
fn normalized(name: &str) -> String {
    let kept: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '/' || c == '.' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let words: Vec<&str> = kept.split_whitespace().collect();
    let mut joined = String::new();
    for (i, word) in words.iter().enumerate() {
        let previous_is_number = i > 0
            && words[i - 1]
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_digit());
        let starts_alpha = word.chars().next().is_some_and(char::is_alphabetic);
        if i > 0 && !(previous_is_number && starts_alpha) {
            joined.push(' ');
        }
        joined.push_str(word);
    }
    joined.replace(" / ", "/")
}

fn one_or_many(mut found: Vec<&Product>, by_code: bool) -> Resolution {
    found.sort_by(|a, b| a.code.cmp(&b.code));
    found.dedup_by(|a, b| a.code == b.code);
    match found.as_slice() {
        [] => Resolution::Unknown,
        [only] if by_code => Resolution::ByCode {
            product: only.code.clone(),
        },
        [only] => Resolution::ByName {
            product: only.code.clone(),
        },
        many => Resolution::Ambiguous {
            products: many.iter().map(|p| p.code.clone()).collect(),
        },
    }
}

/// The stock product `medicine` is dispensed as, among the active `products`.
pub fn resolve(products: &[Product], medicine: &Medicine) -> Resolution {
    let active: Vec<&Product> = products.iter().filter(|p| p.active).collect();
    if let Some(coding) = &medicine.coding {
        let by_code: Vec<&Product> = active
            .iter()
            .copied()
            .filter(|p| p.medicines.contains(coding))
            .collect();
        if !by_code.is_empty() {
            return one_or_many(by_code, true);
        }
    }
    let wanted = normalized(&medicine.text);
    if wanted.is_empty() {
        return Resolution::Unknown;
    }
    let by_name: Vec<&Product> = active
        .into_iter()
        .filter(|p| {
            std::iter::once(&p.name)
                .chain(&p.synonyms)
                .any(|n| normalized(n) == wanted)
        })
        .collect();
    one_or_many(by_name, false)
}

#[cfg(test)]
mod tests;
