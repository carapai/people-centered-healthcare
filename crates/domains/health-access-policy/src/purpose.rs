//! The declared purpose of use (ADR 0056): an `X-Purpose-Of-Use` header carrying a v3
//! PurposeOfUse code, `TREAT` when absent.

use crate::decision::PurposeOfUse;

/// The purpose when a request declares none.
pub const DEFAULT_PURPOSE: &str = "TREAT";

/// The v3 PurposeOfUse codes (`http://terminology.hl7.org/CodeSystem/v3-ActReason`, value set
/// `v3-PurposeOfUse`) a request may declare.
pub const PURPOSES_OF_USE: &[&str] = &[
    "BTG",
    "CAREMGT",
    "CLINTRCH",
    "CLMATTCH",
    "COVAUTH",
    "COVERAGE",
    "DISASTER",
    "ELIGDTRM",
    "ELIGVER",
    "ENROLLM",
    "ETREAT",
    "FAMRQT",
    "HACCRED",
    "HCOMPL",
    "HDIRECT",
    "HLEGAL",
    "HMARKT",
    "HOPERAT",
    "HPAYMT",
    "HQUALIMP",
    "HRESCH",
    "HSYSADMIN",
    "HTEST",
    "METAMGT",
    "PATRQT",
    "POPHLTH",
    "PUBHLTH",
    "PWATRNY",
    "REMITADV",
    "SUPNWK",
    "SYSDEV",
    "THREAT",
    "TRAIN",
    "TREAT",
];

/// A declared purpose that is not a v3 PurposeOfUse code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a v3 PurposeOfUse code")]
pub struct UnknownPurpose(pub String);

/// The purpose of use a request declares in `header`, or [`DEFAULT_PURPOSE`] without one.
pub fn purpose_of_use(header: Option<&str>) -> Result<PurposeOfUse, UnknownPurpose> {
    let code = header.map_or(DEFAULT_PURPOSE, str::trim);
    if PURPOSES_OF_USE.contains(&code) {
        Ok(code.to_owned())
    } else {
        Err(UnknownPurpose(code.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purposes_are_v3_codes_defaulting_to_treat() {
        assert_eq!(purpose_of_use(None), Ok("TREAT".into()));
        assert_eq!(purpose_of_use(Some(" HPAYMT ")), Ok("HPAYMT".into()));
        assert_eq!(
            purpose_of_use(Some("treat")),
            Err(UnknownPurpose("treat".into()))
        );
        assert!(purpose_of_use(Some("")).is_err());
        assert!(PURPOSES_OF_USE.windows(2).all(|w| w[0] < w[1]), "sorted");
    }
}
