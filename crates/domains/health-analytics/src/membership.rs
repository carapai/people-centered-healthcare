//! Membership in another indicator (studio.md § Indicators, *+ in another indicator*; #550): a
//! criterion may require a patient to be in another indicator's numerator or denominator, pinned
//! to that indicator's `url|version`.
//!
//! The condition tree writes it as one FHIRPath expression,
//! `'<url>|<version>' in %indicators.numerator` (or `.denominator`), so the criteria stay one
//! expression and the reference can be read back from it ([`references`]). Evaluation binds
//! `%indicators` for each patient to the indicators they are in, by side. Indicators that refer
//! to each other in a circle cannot be evaluated, and are refused at Validation ([`circles`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The variable a criterion reads another indicator's membership from: `%indicators`.
pub const MEMBERSHIP_VARIABLE: &str = "indicators";

/// A side of another indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    /// Its numerator: what it counts, when it has no denominator.
    Numerator,
    /// Its denominator.
    Denominator,
}

impl Side {
    /// What a name names.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "numerator" => Self::Numerator,
            "denominator" => Self::Denominator,
            _ => return None,
        })
    }

    /// Its name, as `%indicators` holds it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Numerator => "numerator",
            Self::Denominator => "denominator",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A criterion's reference to another indicator's side.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference {
    /// The other indicator, `url|version`.
    pub measure: String,
    /// Which side of it.
    pub side: Side,
}

/// The FHIRPath a reference compiles to.
pub fn compile(reference: &Reference) -> String {
    format!(
        "'{}' in %{MEMBERSHIP_VARIABLE}.{}",
        reference.measure, reference.side
    )
}

/// Every reference to another indicator in `expression`, or why one is not usable: each use of
/// `%indicators` must be `'<url>|<version>' in %indicators.numerator` or `.denominator`.
pub fn references(expression: &str) -> Result<BTreeSet<Reference>, String> {
    let variable = format!("%{MEMBERSHIP_VARIABLE}");
    let mut found = BTreeSet::new();
    for (at, _) in expression.match_indices(&variable) {
        let after = &expression[at + variable.len()..];
        let side = after
            .strip_prefix('.')
            .map(|rest| {
                rest.split(|c: char| !c.is_ascii_alphanumeric())
                    .next()
                    .unwrap_or_default()
            })
            .and_then(Side::from_name);
        let measure = expression[..at]
            .strip_suffix("' in ")
            .and_then(|before| before.rsplit_once('\'').map(|(_, quoted)| quoted));
        let (Some(side), Some(measure)) = (side, measure) else {
            return Err(format!(
                "reads %{MEMBERSHIP_VARIABLE} other than as '<url>|<version>' in %{MEMBERSHIP_VARIABLE}.numerator or .denominator"
            ));
        };
        match measure.split_once('|') {
            Some((url, version)) if !url.is_empty() && !version.is_empty() => {}
            _ => {
                return Err(format!(
                    "refers to the indicator {measure}, which is not pinned to a version (url|version)"
                ));
            }
        }
        found.insert(Reference {
            measure: measure.to_owned(),
            side,
        });
    }
    Ok(found)
}

/// Every circle among indicators that refer to others: each set of indicators (by
/// `url|version`) that reach one another through their references, including one that refers to
/// itself. `refers` maps each indicator to those it refers to; indicators it does not hold are
/// referred to but not followed.
pub fn circles(refers: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    // Tarjan's strongly connected components, iteratively deep enough for a package.
    struct Walk<'a> {
        refers: &'a BTreeMap<String, BTreeSet<String>>,
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        stack: Vec<&'a str>,
        on_stack: BTreeSet<&'a str>,
        found: Vec<Vec<String>>,
    }
    impl<'a> Walk<'a> {
        fn visit(&mut self, node: &'a str) {
            let n = self.index.len();
            self.index.insert(node, n);
            self.low.insert(node, n);
            self.stack.push(node);
            self.on_stack.insert(node);
            for next in self.refers.get(node).into_iter().flatten() {
                let next = next.as_str();
                if !self.refers.contains_key(next) {
                    continue;
                }
                if !self.index.contains_key(next) {
                    self.visit(next);
                    let low = self.low[node].min(self.low[next]);
                    self.low.insert(node, low);
                } else if self.on_stack.contains(next) {
                    let low = self.low[node].min(self.index[next]);
                    self.low.insert(node, low);
                }
            }
            if self.low[node] == self.index[node] {
                let mut component = Vec::new();
                while let Some(top) = self.stack.pop() {
                    self.on_stack.remove(top);
                    component.push(top.to_owned());
                    if top == node {
                        break;
                    }
                }
                let to_itself = self.refers.get(node).is_some_and(|r| r.contains(node));
                if component.len() > 1 || to_itself {
                    component.sort();
                    self.found.push(component);
                }
            }
        }
    }
    let mut walk = Walk {
        refers,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        found: Vec::new(),
    };
    for node in refers.keys() {
        if !walk.index.contains_key(node.as_str()) {
            walk.visit(node);
        }
    }
    walk.found.sort();
    walk.found
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &str = "https://health.go.ug/fhir/Measure/anc-first-visits|1.0.0";

    fn reference(measure: &str, side: Side) -> Reference {
        Reference {
            measure: measure.to_owned(),
            side,
        }
    }

    #[test]
    fn a_reference_compiles_and_reads_back() {
        let numerator = reference(FIRST, Side::Numerator);
        let denominator = reference("https://x/Measure/b|2.1.0", Side::Denominator);
        let expression = format!(
            "{} and ({}).not() and %patient.gender = 'female'",
            compile(&numerator),
            compile(&denominator)
        );
        assert_eq!(
            compile(&numerator),
            format!("'{FIRST}' in %indicators.numerator")
        );
        assert_eq!(
            references(&expression),
            Ok(BTreeSet::from([numerator, denominator]))
        );
        assert_eq!(
            references("%patient.gender = 'female'"),
            Ok(BTreeSet::new())
        );
    }

    #[test]
    fn a_reference_is_pinned_and_written_as_the_tree_writes_it() {
        assert!(
            references("'https://x/Measure/a' in %indicators.numerator")
                .expect_err("unpinned")
                .contains("not pinned to a version")
        );
        for other in [
            "%indicators.numerator.exists()",
            "'https://x/Measure/a|1' in %indicators.initial",
            "'https://x/Measure/a|1' in %indicators",
        ] {
            assert!(
                references(other)
                    .expect_err(other)
                    .contains("other than as"),
                "{other}"
            );
        }
    }

    fn graph(edges: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
        edges
            .iter()
            .map(|(from, to)| {
                (
                    (*from).to_owned(),
                    to.iter().map(|t| (*t).to_owned()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn indicators_that_refer_to_each_other_are_a_circle() {
        assert_eq!(
            circles(&graph(&[
                ("a|1", &["b|1"]),
                ("b|1", &["a|1"]),
                ("c|1", &["a|1"])
            ])),
            vec![vec!["a|1".to_owned(), "b|1".to_owned()]]
        );
        assert_eq!(
            circles(&graph(&[("a|1", &["a|1"])])),
            vec![vec!["a|1".to_owned()]]
        );
        assert_eq!(
            circles(&graph(&[
                ("a|1", &["b|1"]),
                ("b|1", &["c|1"]),
                ("c|1", &["a|1"]),
                ("d|1", &["e|1"]),
                ("e|1", &["d|1"]),
            ])),
            vec![
                vec!["a|1".to_owned(), "b|1".to_owned(), "c|1".to_owned()],
                vec!["d|1".to_owned(), "e|1".to_owned()],
            ]
        );
    }

    #[test]
    fn a_chain_or_a_reference_outside_is_no_circle() {
        assert!(
            circles(&graph(&[
                ("a|1", &["b|1", "outside|1"]),
                ("b|1", &["c|1"]),
                ("c|1", &[]),
                ("d|1", &["b|1"]),
            ]))
            .is_empty()
        );
    }
}
