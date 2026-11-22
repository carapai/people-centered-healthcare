//! Threshold bands (analytics.md § Dashboards, *Thresholds*; ADR 0075; #600): an indicator may
//! carry a legend set, ranges of its figure each read as good, warning or bad, and every
//! Visualization colours a figure by the band it falls in.
//!
//! - **A band** is `from` (inclusive) up to `to` (exclusive); a band without `from` runs from
//!   below every figure, one without `to` above every figure. Bands may leave gaps, where a
//!   figure takes no colour, but never overlap, so a figure falls in at most one band.
//! - **Only colour.** A band says nothing to anyone: alerting when an outbreak threshold is
//!   crossed is surveillance's (ADR 0091), from the Surveillance package's own rules.

use std::str::FromStr;

use rust_decimal::Decimal;

/// How a figure in a band reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tone {
    /// On target.
    Good,
    /// Needs watching.
    Warning,
    /// Needs action.
    Bad,
}

impl Tone {
    /// The tone a code names: `good`, `warning` or `bad`.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "good" => Self::Good,
            "warning" => Self::Warning,
            "bad" => Self::Bad,
            _ => return None,
        })
    }

    /// Its code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Warning => "warning",
            Self::Bad => "bad",
        }
    }
}

/// A range of an indicator's figure and how a figure in it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdBand {
    /// The lowest figure in it; none is below every figure.
    pub from: Option<Decimal>,
    /// The first figure above it; none is above every figure.
    pub to: Option<Decimal>,
    /// How a figure in it reads.
    pub tone: Tone,
}

impl ThresholdBand {
    /// Whether `value` is in this band.
    pub fn holds(&self, value: Decimal) -> bool {
        self.from.is_none_or(|from| value >= from) && self.to.is_none_or(|to| value < to)
    }
}

/// How `value` reads by `bands`: the tone of the band it falls in, if any.
pub fn tone_of(bands: &[ThresholdBand], value: Decimal) -> Option<Tone> {
    bands.iter().find(|b| b.holds(value)).map(|b| b.tone)
}

/// A threshold band, as written: each part as text, so that Validation can name what is wrong.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThresholdDefinition {
    /// Its lowest figure, as a decimal.
    pub from: Option<String>,
    /// The first figure above it, as a decimal.
    pub to: Option<String>,
    /// Its tone's code.
    pub tone: Option<String>,
}

/// The bands `written`, lowest first, or every reason they are not a legend set.
pub fn threshold_bands(written: &[ThresholdDefinition]) -> Result<Vec<ThresholdBand>, Vec<String>> {
    let mut problems = Vec::new();
    let mut bands = Vec::new();
    for (n, band) in written.iter().enumerate() {
        let row = n + 1;
        let before = problems.len();
        let mut bound = |value: &Option<String>, what: &str| match value.as_deref() {
            None => None,
            Some(text) => Decimal::from_str(text.trim())
                .map(Some)
                .unwrap_or_else(|_| {
                    problems.push(format!(
                        "threshold band {row}'s {what} {text} is not a number"
                    ));
                    None
                }),
        };
        let from = bound(&band.from, "from");
        let to = bound(&band.to, "to");
        let tone = match band.tone.as_deref() {
            None => {
                problems.push(format!(
                    "threshold band {row} has no tone (good, warning or bad)"
                ));
                None
            }
            Some(code) => Tone::from_code(code).or_else(|| {
                problems.push(format!(
                    "threshold band {row}'s tone {code} is not good, warning or bad"
                ));
                None
            }),
        };
        if let (Some(from), Some(to)) = (from, to)
            && from >= to
        {
            problems.push(format!(
                "threshold band {row} runs from {from} to {to}: its from must be below its to"
            ));
        }
        // Only a band read without problems is checked against the others.
        if let Some(tone) = tone.filter(|_| problems.len() == before) {
            bands.push(ThresholdBand { from, to, tone });
        }
    }
    bands.sort_by_key(|b| b.from);
    for pair in bands.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        let overlaps = match (low.to, high.from) {
            (Some(to), Some(from)) => to > from,
            _ => true,
        };
        if overlaps {
            problems.push(format!(
                "threshold bands overlap: {} and {}",
                range(&low),
                range(&high)
            ));
        }
    }
    if problems.is_empty() {
        Ok(bands)
    } else {
        Err(problems)
    }
}

/// A band's range as a person reads it: `20 to 40`, `below 20`, `40 and above`.
pub fn range(band: &ThresholdBand) -> String {
    match (band.from, band.to) {
        (Some(from), Some(to)) => format!("{from} to {to}"),
        (None, Some(to)) => format!("below {to}"),
        (Some(from), None) => format!("{from} and above"),
        (None, None) => "every figure".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(from: Option<&str>, to: Option<&str>, tone: &str) -> ThresholdDefinition {
        ThresholdDefinition {
            from: from.map(Into::into),
            to: to.map(Into::into),
            tone: Some(tone.into()),
        }
    }

    /// Malaria test positivity: under 20% good, 20–40% warning, 40% and above bad.
    fn malaria_positivity() -> Vec<ThresholdDefinition> {
        vec![
            band(Some("40"), None, "bad"),
            band(None, Some("20"), "good"),
            band(Some("20"), Some("40"), "warning"),
        ]
    }

    #[test]
    fn a_figure_takes_the_tone_of_the_band_it_falls_in() {
        let bands = threshold_bands(&malaria_positivity()).expect("bands");
        assert_eq!(
            bands.iter().map(|b| b.tone).collect::<Vec<_>>(),
            [Tone::Good, Tone::Warning, Tone::Bad],
            "lowest first"
        );
        assert_eq!(tone_of(&bands, Decimal::new(45, 0)), Some(Tone::Bad));
        assert_eq!(tone_of(&bands, Decimal::new(40, 0)), Some(Tone::Bad));
        assert_eq!(tone_of(&bands, Decimal::new(3999, 2)), Some(Tone::Warning));
        assert_eq!(tone_of(&bands, Decimal::new(-1, 0)), Some(Tone::Good));
    }

    #[test]
    fn a_gap_leaves_a_figure_uncoloured() {
        let bands = threshold_bands(&[
            band(None, Some("50"), "bad"),
            band(Some("80"), None, "good"),
        ])
        .expect("bands");
        assert_eq!(tone_of(&bands, Decimal::new(60, 0)), None);
        assert_eq!(tone_of(&bands, Decimal::new(80, 0)), Some(Tone::Good));
    }

    #[test]
    fn overlapping_or_unreadable_bands_are_refused() {
        assert_eq!(
            threshold_bands(&[
                band(None, Some("30"), "good"),
                band(Some("20"), None, "bad")
            ]),
            Err(vec![
                "threshold bands overlap: below 30 and 20 and above".to_owned()
            ])
        );
        assert_eq!(
            threshold_bands(&[
                band(Some("ten"), None, "good"),
                band(Some("5"), Some("5"), "bad"),
                ThresholdDefinition {
                    tone: Some("red".into()),
                    ..ThresholdDefinition::default()
                },
                ThresholdDefinition::default(),
            ]),
            Err(vec![
                "threshold band 1's from ten is not a number".to_owned(),
                "threshold band 2 runs from 5 to 5: its from must be below its to".to_owned(),
                "threshold band 3's tone red is not good, warning or bad".to_owned(),
                "threshold band 4 has no tone (good, warning or bad)".to_owned(),
            ])
        );
    }

    #[test]
    fn tones_read_back() {
        for tone in [Tone::Good, Tone::Warning, Tone::Bad] {
            assert_eq!(Tone::from_code(tone.code()), Some(tone));
        }
    }
}
