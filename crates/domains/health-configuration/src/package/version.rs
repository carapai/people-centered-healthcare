//! Exact semantic versions (semver 2.0.0): what publication pins dependencies to.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// An exact version, `MAJOR.MINOR.PATCH` with an optional pre-release. Build metadata is
/// refused: two packages differing only in it would have one `name@version`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Version {
    /// Breaking changes.
    pub major: u64,
    /// Additive changes.
    pub minor: u64,
    /// Changes with no change of meaning.
    pub patch: u64,
    /// The pre-release identifiers, dot-separated, if any.
    pub pre: Option<String>,
}

/// Not an exact version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not an exact version (MAJOR.MINOR.PATCH)")]
pub struct NotExact(pub String);

fn number(part: &str) -> Option<u64> {
    let leading_zero = part.len() > 1 && part.starts_with('0');
    if part.is_empty() || leading_zero || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

fn identifier(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && (!part.bytes().all(|b| b.is_ascii_digit()) || number(part).is_some())
}

impl FromStr for Version {
    type Err = NotExact;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let refused = || NotExact(text.to_owned());
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        if let Some(pre) = pre
            && !pre.split('.').all(identifier)
        {
            return Err(refused());
        }
        let mut parts = core.split('.');
        let (Some(major), Some(minor), Some(patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(refused());
        };
        Ok(Self {
            major: number(major).ok_or_else(refused)?,
            minor: number(minor).ok_or_else(refused)?,
            patch: number(patch).ok_or_else(refused)?,
            pre: pre.map(str::to_owned),
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        match &self.pre {
            Some(pre) => write!(f, "-{pre}"),
            None => Ok(()),
        }
    }
}

impl serde::Serialize for Version {
    /// As written: `1.2.3-rc.1`.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    /// Only an exact version.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl Ord for Version {
    /// Semver precedence: a pre-release sorts before its release.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => pre_release(a, b),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn pre_release(a: &str, b: &str) -> Ordering {
    let mut left = a.split('.');
    let mut right = b.split('.');
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (number(x), number(y)) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_versions_parse_and_ranges_do_not() {
        let v: Version = "1.4.0".parse().expect("exact");
        assert_eq!((v.major, v.minor, v.patch, v.pre), (1, 4, 0, None));
        assert_eq!(
            "2.0.0-rc.1".parse::<Version>().map(|v| v.to_string()),
            Ok("2.0.0-rc.1".to_owned())
        );
        for range in [
            "^1.4.0",
            "~1.4.0",
            "1.4",
            "1.x",
            ">=1.0.0",
            "*",
            "1.4.0 - 2.0.0",
            "01.4.0",
            "1.4.0+build",
            "1.4.0-",
            "1.4.0-01",
            "latest",
            "",
        ] {
            assert_eq!(
                range.parse::<Version>(),
                Err(NotExact(range.to_owned())),
                "{range}"
            );
        }
    }

    #[test]
    fn precedence_follows_semver() {
        let order = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.10.0",
            "2.0.0",
        ]
        .map(|v| v.parse::<Version>().expect("exact"));
        assert!(order.windows(2).all(|w| w[0] < w[1]));
    }
}
