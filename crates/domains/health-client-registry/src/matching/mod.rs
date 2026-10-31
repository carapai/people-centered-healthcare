//! Patient matching (ADR 0028; ADR 0055; patient-identity.md § Matching): a deterministic
//! short-circuit on verified identifiers, blocking on narrow keys with a stop-list,
//! Fellegi–Sunter scoring with term-frequency adjustment and explicit null handling, and
//! classification. Everything tunable is in [`MatchingRules`], a published definition.

mod compare;

use std::collections::BTreeSet;

pub use compare::{damerau_levenshtein, jaro_winkler};

/// The phonetic encoder the rules name (Double Metaphone primary codes by default). Encoding
/// lives outside the domain; this is its port.
pub trait Phonetic {
    /// The primary code of `word`, at most `length` characters.
    fn encode(&self, word: &str, length: usize) -> String;
}

/// An identifier on a person: a NIN, an ECID or a program number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    /// Its system.
    pub system: String,
    /// Its value.
    pub value: String,
    /// Verified (by NIRA, or minted by the registry or program): only verified identifiers
    /// decide deterministically.
    pub verified: bool,
}

/// The demographics matching compares: a `$match` query or a stored source Patient.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Person {
    /// The Patient id, for stored people.
    pub id: Option<String>,
    /// Family name.
    pub family: Option<String>,
    /// Given names.
    pub given: Vec<String>,
    /// Birth year.
    pub birth_year: Option<i32>,
    /// Whether the birth date is an estimate.
    pub birth_estimated: bool,
    /// Administrative gender.
    pub sex: Option<String>,
    /// District of residence.
    pub district: Option<String>,
    /// Mobile phone.
    pub phone: Option<String>,
    /// Identifiers, verified or not.
    pub identifiers: Vec<Identifier>,
}

/// Folds a name or place for comparison: uppercase letters only.
pub fn fold(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// A phone's comparable form: its last nine digits (the Ugandan subscriber number).
pub fn phone_key(phone: &str) -> String {
    let digits: Vec<char> = phone.chars().filter(char::is_ascii_digit).collect();
    digits[digits.len().saturating_sub(9)..].iter().collect()
}

// ---------------------------------------------------------------------------------------------
// Rules

/// A blocking key (ADR 0055).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyKind {
    /// The NIN.
    Nin,
    /// The normalized phone (low weight; shared in households).
    Phone,
    /// Phonetic family + birth year + district.
    FamYobDist,
    /// Phonetic family and given codes, sorted, + birth year.
    FamGivenYob,
    /// District + phonetic given + birth year + sex.
    DistGivenYobSex,
}

impl KeyKind {
    /// The key's name in the rules and in the blocking index.
    pub fn code(self) -> &'static str {
        match self {
            Self::Nin => "nin",
            Self::Phone => "phone",
            Self::FamYobDist => "fam_yob_dist",
            Self::FamGivenYob => "fam_given_yob",
            Self::DistGivenYobSex => "dist_given_yob_sex",
        }
    }
}

/// A compared field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MatchField {
    /// Family name.
    Family,
    /// First given name.
    Given,
    /// Birth year.
    BirthYear,
    /// Sex.
    Sex,
    /// District.
    District,
    /// Phone.
    Phone,
    /// NIN (unverified values; verified ones decide deterministically).
    Nin,
}

/// How a field's values are compared.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comparator {
    /// Equal after folding.
    Exact,
    /// Jaro–Winkler similarity at least this.
    JaroWinkler(f64),
    /// At most this many edits.
    DamerauLevenshtein(usize),
    /// Same phonetic code.
    Phonetic,
    /// Years at most this far apart (more when the query's birth date is estimated: ±5).
    YearWithin(i32),
}

/// One field's comparator and m/u probabilities (calibrated with Splink EM, then published).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldRule {
    /// The field.
    pub field: MatchField,
    /// How values are compared.
    pub comparator: Comparator,
    /// P(agree | same person).
    pub m: f64,
    /// P(agree | different people), used when no term frequency is known.
    pub u: f64,
    /// Whether agreement on a value uses that value's frequency as `u` when it is higher, so a
    /// frequent surname or a phone many people share weighs less. A rare value never weighs
    /// more than `u` gives.
    pub term_frequency: bool,
}

/// What a missing value on either side contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullHandling {
    /// As a disagreement: missing data never helps a link.
    Conservative,
    /// Half a disagreement.
    Moderate,
    /// Nothing.
    Greedy,
}

/// The Matching Rules (ADR 0028): a versioned published definition.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchingRules {
    /// `url|version` of the published definition.
    pub version: String,
    /// The identifier systems that decide deterministically when verified (NIN, ECID, program
    /// numbers).
    pub verified_systems: Vec<String>,
    /// The NIN's system, for the `nin` key and field.
    pub nin_system: String,
    /// The blocking keys in use.
    pub keys: Vec<KeyKind>,
    /// Phonetic code length.
    pub phonetic_length: usize,
    /// A block larger than this is skipped (the stop-list).
    pub max_block: u64,
    /// The scored fields.
    pub fields: Vec<FieldRule>,
    /// Missing values.
    pub null_handling: NullHandling,
    /// At or above: auto-link (set to measured precision ≥ 99.9%).
    pub upper: f64,
    /// At or above (and below `upper`): potential match, for review.
    pub lower: f64,
}

impl MatchingRules {
    /// The platform defaults (ADR 0055 keys). The m/u values are placeholders until the Splink
    /// calibration publishes real ones, and the thresholds are deliberately conservative.
    pub fn platform_default(nin_system: &str, verified_systems: &[&str]) -> Self {
        let field = |field, comparator, m, u, term_frequency| FieldRule {
            field,
            comparator,
            m,
            u,
            term_frequency,
        };
        Self {
            version: "urn:health:matching-rules|0.1.0".into(),
            verified_systems: verified_systems.iter().map(|s| (*s).to_owned()).collect(),
            nin_system: nin_system.to_owned(),
            keys: vec![
                KeyKind::Nin,
                KeyKind::Phone,
                KeyKind::FamYobDist,
                KeyKind::FamGivenYob,
                KeyKind::DistGivenYobSex,
            ],
            phonetic_length: 6,
            max_block: 1_000,
            fields: vec![
                field(
                    MatchField::Family,
                    Comparator::JaroWinkler(0.92),
                    0.95,
                    0.01,
                    true,
                ),
                field(
                    MatchField::Given,
                    Comparator::JaroWinkler(0.92),
                    0.92,
                    0.02,
                    true,
                ),
                field(
                    MatchField::BirthYear,
                    Comparator::YearWithin(1),
                    0.95,
                    0.06,
                    false,
                ),
                field(MatchField::Sex, Comparator::Exact, 0.98, 0.5, false),
                field(MatchField::District, Comparator::Exact, 0.85, 0.05, false),
                // Household members share phones: a low weight (ADR 0028).
                field(MatchField::Phone, Comparator::Exact, 0.4, 0.05, true),
                // Unverified, but two different NINs are strong evidence of two people.
                field(
                    MatchField::Nin,
                    Comparator::DamerauLevenshtein(1),
                    0.98,
                    0.0001,
                    false,
                ),
            ],
            null_handling: NullHandling::Conservative,
            // Same-sex twins in one household agree on everything but the given name and score
            // about 18; the auto-link threshold stays above them.
            upper: 20.0,
            lower: 8.0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Deterministic

/// The deterministic layer's answer for a pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deterministic {
    /// They share a verified identifier: the same person.
    Match,
    /// Different verified values in the same system: never the same person, whatever the score.
    Conflict,
    /// Nothing verified decides; score them.
    Undecided,
}

/// Compares the pair's verified identifiers in the rules' systems. A conflict outranks a match.
pub fn deterministic(a: &Person, b: &Person, rules: &MatchingRules) -> Deterministic {
    let verified = |p: &Person| -> Vec<(String, String)> {
        p.identifiers
            .iter()
            .filter(|i| i.verified && rules.verified_systems.contains(&i.system))
            .map(|i| (i.system.clone(), i.value.trim().to_uppercase()))
            .collect()
    };
    let (va, vb) = (verified(a), verified(b));
    let mut shared = false;
    for (system, value) in &va {
        let theirs: Vec<&String> = vb
            .iter()
            .filter(|(s, _)| s == system)
            .map(|(_, v)| v)
            .collect();
        if theirs.is_empty() {
            continue;
        }
        if theirs.contains(&value) {
            shared = true;
        } else {
            return Deterministic::Conflict;
        }
    }
    if shared {
        Deterministic::Match
    } else {
        Deterministic::Undecided
    }
}

// ---------------------------------------------------------------------------------------------
// Blocking

/// One blocking key value, as stored in `idx_block:[kind, value, patient]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockKey {
    /// The key.
    pub kind: KeyKind,
    /// Its value.
    pub value: String,
}

fn nin_of<'a>(person: &'a Person, rules: &MatchingRules) -> Option<&'a str> {
    person
        .identifiers
        .iter()
        .find(|i| i.system == rules.nin_system)
        .map(|i| i.value.as_str())
}

/// The keys of `person` for the given birth years (one for storage; a range for queries).
fn keys_for(
    person: &Person,
    phonetic: &dyn Phonetic,
    rules: &MatchingRules,
    years: &dyn Fn(KeyKind) -> Vec<i32>,
) -> Vec<BlockKey> {
    let code = |word: &str| phonetic.encode(&fold(word), rules.phonetic_length);
    let family = person
        .family
        .as_deref()
        .map(fold)
        .filter(|f| !f.is_empty())
        .map(|f| code(&f));
    let given = person
        .given
        .first()
        .map(|g| fold(g))
        .filter(|g| !g.is_empty())
        .map(|g| code(&g));
    let district = person
        .district
        .as_deref()
        .map(fold)
        .filter(|d| !d.is_empty());
    let sex = person.sex.as_deref().map(str::to_lowercase);
    let mut keys = BTreeSet::new();
    for kind in &rules.keys {
        let mut push = |value: String| {
            keys.insert(BlockKey { kind: *kind, value });
        };
        match kind {
            KeyKind::Nin => {
                if let Some(nin) = nin_of(person, rules) {
                    push(nin.trim().to_uppercase());
                }
            }
            KeyKind::Phone => {
                if let Some(phone) = person
                    .phone
                    .as_deref()
                    .map(phone_key)
                    .filter(|p| p.len() == 9)
                {
                    push(phone);
                }
            }
            KeyKind::FamYobDist => {
                if let (Some(f), Some(d)) = (&family, &district) {
                    for y in years(*kind) {
                        push(format!("{f}|{y}|{d}"));
                    }
                }
            }
            KeyKind::FamGivenYob => {
                if let (Some(f), Some(g)) = (&family, &given) {
                    let mut pair = [f.as_str(), g.as_str()];
                    pair.sort_unstable();
                    for y in years(*kind) {
                        push(format!("{}|{}|{y}", pair[0], pair[1]));
                    }
                }
            }
            KeyKind::DistGivenYobSex => {
                if let (Some(d), Some(g), Some(s)) = (&district, &given, &sex) {
                    for y in years(*kind) {
                        push(format!("{d}|{g}|{y}|{s}"));
                    }
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// The keys a stored person is indexed under (exact birth year).
pub fn index_keys(
    person: &Person,
    phonetic: &dyn Phonetic,
    rules: &MatchingRules,
) -> Vec<BlockKey> {
    let year = person.birth_year;
    keys_for(person, phonetic, rules, &|_| year.into_iter().collect())
}

/// The keys a query looks up: birth year ±1 for `fam_given_yob`, or ±5 on every year key when
/// the birth date is estimated (ADR 0055).
pub fn query_keys(
    person: &Person,
    phonetic: &dyn Phonetic,
    rules: &MatchingRules,
) -> Vec<BlockKey> {
    let year = person.birth_year;
    let estimated = person.birth_estimated;
    keys_for(person, phonetic, rules, &move |kind| {
        let Some(y) = year else { return Vec::new() };
        let spread = if estimated {
            5
        } else if kind == KeyKind::FamGivenYob {
            1
        } else {
            0
        };
        (y - spread..=y + spread).collect()
    })
}

/// Which keys a query may use, given each block's stored count (the hourly `block_count`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPlan {
    /// Keys to look up.
    pub used: Vec<BlockKey>,
    /// Keys skipped because their block exceeds `max_block`; their share is a quality metric.
    pub skipped: Vec<BlockKey>,
}

impl BlockPlan {
    /// Whether there is nothing to look up: the query has too little identifying data, and the
    /// caller falls back to a manual registry search.
    pub fn insufficient(&self) -> bool {
        self.used.is_empty()
    }
}

/// Splits `keys` by the stop-list.
pub fn plan(
    keys: Vec<BlockKey>,
    count: &dyn Fn(&BlockKey) -> u64,
    rules: &MatchingRules,
) -> BlockPlan {
    let (used, skipped) = keys.into_iter().partition(|k| count(k) <= rules.max_block);
    BlockPlan { used, skipped }
}

// ---------------------------------------------------------------------------------------------
// Scoring

/// The FHIR `match-grade` of a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchGrade {
    /// A shared verified identifier: auto-link.
    Certain,
    /// Score at or above `upper`: auto-link.
    Probable,
    /// Between the thresholds: registry review.
    Possible,
    /// Below `lower`, or conflicting verified identifiers.
    CertainlyNot,
}

impl MatchGrade {
    /// The `match-grade` code.
    pub fn code(self) -> &'static str {
        match self {
            Self::Certain => "certain",
            Self::Probable => "probable",
            Self::Possible => "possible",
            Self::CertainlyNot => "certainly-not",
        }
    }

    /// Whether this grade links without a reviewer.
    pub fn auto_links(self) -> bool {
        matches!(self, Self::Certain | Self::Probable)
    }
}

/// One evaluated candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    /// The total match weight (log2 likelihood ratio); infinite for deterministic answers.
    pub score: f64,
    /// The classification.
    pub grade: MatchGrade,
}

fn values(person: &Person, field: MatchField, rules: &MatchingRules) -> Option<String> {
    match field {
        MatchField::Family => person.family.as_deref().map(fold).filter(|s| !s.is_empty()),
        MatchField::Given => person
            .given
            .first()
            .map(|g| fold(g))
            .filter(|s| !s.is_empty()),
        MatchField::BirthYear => person.birth_year.map(|y| y.to_string()),
        MatchField::Sex => person.sex.as_deref().map(str::to_lowercase),
        MatchField::District => person
            .district
            .as_deref()
            .map(fold)
            .filter(|s| !s.is_empty()),
        MatchField::Phone => person
            .phone
            .as_deref()
            .map(phone_key)
            .filter(|p| !p.is_empty()),
        MatchField::Nin => nin_of(person, rules).map(|n| n.trim().to_uppercase()),
    }
}

fn agrees(
    rule: &FieldRule,
    a: &str,
    b: &str,
    query: &Person,
    phonetic: &dyn Phonetic,
    rules: &MatchingRules,
) -> bool {
    match rule.comparator {
        Comparator::Exact => a == b,
        Comparator::JaroWinkler(threshold) => jaro_winkler(a, b) >= threshold,
        Comparator::DamerauLevenshtein(max) => damerau_levenshtein(a, b) <= max,
        Comparator::Phonetic => {
            phonetic.encode(a, rules.phonetic_length) == phonetic.encode(b, rules.phonetic_length)
        }
        Comparator::YearWithin(within) => {
            let within = if query.birth_estimated {
                within.max(5)
            } else {
                within
            };
            match (a.parse::<i32>(), b.parse::<i32>()) {
                (Ok(x), Ok(y)) => (x - y).abs() <= within,
                _ => false,
            }
        }
    }
}

/// Scores `candidate` against `query`. `frequency(field, value)` is the share of stored people
/// with that value (term frequency), when known.
pub fn evaluate(
    query: &Person,
    candidate: &Person,
    rules: &MatchingRules,
    phonetic: &dyn Phonetic,
    frequency: &dyn Fn(MatchField, &str) -> Option<f64>,
) -> Evaluation {
    match deterministic(query, candidate, rules) {
        Deterministic::Match => {
            return Evaluation {
                score: f64::INFINITY,
                grade: MatchGrade::Certain,
            };
        }
        Deterministic::Conflict => {
            return Evaluation {
                score: f64::NEG_INFINITY,
                grade: MatchGrade::CertainlyNot,
            };
        }
        Deterministic::Undecided => {}
    }
    let mut score = 0.0;
    for rule in &rules.fields {
        let disagree = ((1.0 - rule.m) / (1.0 - rule.u)).log2();
        match (
            values(query, rule.field, rules),
            values(candidate, rule.field, rules),
        ) {
            (Some(a), Some(b)) => {
                if agrees(rule, &a, &b, query, phonetic, rules) {
                    let u = if rule.term_frequency {
                        frequency(rule.field, &b).map_or(rule.u, |f| f.max(rule.u))
                    } else {
                        rule.u
                    };
                    let weight = (rule.m / u).log2();
                    // Agreement only inside an estimated age's ±5 band is weaker evidence.
                    let widened = matches!(rule.comparator, Comparator::YearWithin(w) if query.birth_estimated
                        && matches!((a.parse::<i32>(), b.parse::<i32>()), (Ok(x), Ok(y)) if (x - y).abs() > w));
                    score += if widened { weight / 2.0 } else { weight };
                } else {
                    score += disagree;
                }
            }
            _ => {
                score += match rules.null_handling {
                    NullHandling::Conservative => disagree,
                    NullHandling::Moderate => disagree / 2.0,
                    NullHandling::Greedy => 0.0,
                };
            }
        }
    }
    let grade = if score >= rules.upper {
        MatchGrade::Probable
    } else if score >= rules.lower {
        MatchGrade::Possible
    } else {
        MatchGrade::CertainlyNot
    };
    Evaluation { score, grade }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in encoder: the first `length` consonants.
    struct Consonants;

    impl Phonetic for Consonants {
        fn encode(&self, word: &str, length: usize) -> String {
            word.chars()
                .filter(|c| !"AEIOU".contains(*c))
                .take(length)
                .collect()
        }
    }

    const NIN: &str = "urn:nin";

    fn rules() -> MatchingRules {
        MatchingRules::platform_default(NIN, &[NIN, "urn:health:ecid"])
    }

    fn person(family: &str, given: &str, year: i32, nin: Option<(&str, bool)>) -> Person {
        Person {
            id: None,
            family: Some(family.into()),
            given: vec![given.into()],
            birth_year: Some(year),
            birth_estimated: false,
            sex: Some("female".into()),
            district: Some("Gulu".into()),
            phone: Some("+256 772 123 456".into()),
            identifiers: nin
                .map(|(v, verified)| Identifier {
                    system: NIN.into(),
                    value: v.into(),
                    verified,
                })
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn conflicting_verified_ids_are_hard_non_matches() {
        let a = person("Akello", "Grace", 1990, Some(("CF90AAAA", true)));
        let identical_but_other_nin = person("Akello", "Grace", 1990, Some(("CF90BBBB", true)));
        let e = evaluate(
            &a,
            &identical_but_other_nin,
            &rules(),
            &Consonants,
            &|_, _| None,
        );
        assert_eq!(e.grade, MatchGrade::CertainlyNot);
        assert_eq!(
            deterministic(&a, &a.clone(), &rules()),
            Deterministic::Match
        );
        // Unverified identifiers never decide.
        let unverified = person("Akello", "Grace", 1990, Some(("CF90BBBB", false)));
        assert_eq!(
            deterministic(&a, &unverified, &rules()),
            Deterministic::Undecided
        );
        // A conflict outranks a shared identifier in another system.
        let mut both = a.clone();
        both.identifiers.push(Identifier {
            system: "urn:health:ecid".into(),
            value: "X".into(),
            verified: true,
        });
        let mut other = identical_but_other_nin.clone();
        other.identifiers.push(Identifier {
            system: "urn:health:ecid".into(),
            value: "X".into(),
            verified: true,
        });
        assert_eq!(
            deterministic(&both, &other, &rules()),
            Deterministic::Conflict
        );
    }

    #[test]
    fn scores_rank_the_same_person_above_a_namesake() {
        let rules = rules();
        let query = person("Akello", "Grace", 1990, None);
        let typo = person("Akelo", "Grace", 1991, None);
        let mut namesake = person("Akello", "Grace", 1975, None);
        namesake.district = Some("Lira".into());
        namesake.phone = Some("+256 700 000 001".into());
        let same = evaluate(&query, &typo, &rules, &Consonants, &|_, _| None);
        let other = evaluate(&query, &namesake, &rules, &Consonants, &|_, _| None);
        assert!(
            same.score > other.score,
            "{} vs {}",
            same.score,
            other.score
        );
        // Without a NIN on either side the duplicate goes to review, never straight to a link...
        assert_eq!(same.grade, MatchGrade::Possible, "{same:?}");
        assert!(!other.grade.auto_links());
        // ...and with the same (unverified) NIN it links.
        let with_nin = |p: &Person| Person {
            identifiers: vec![Identifier {
                system: NIN.into(),
                value: "CF90AAAA".into(),
                verified: false,
            }],
            ..p.clone()
        };
        let linked = evaluate(
            &with_nin(&query),
            &with_nin(&typo),
            &rules,
            &Consonants,
            &|_, _| None,
        );
        assert!(linked.grade.auto_links(), "{linked:?}");
        // A frequent surname weighs less.
        let common = evaluate(&query, &typo, &rules, &Consonants, &|f, _| {
            (f == MatchField::Family).then_some(0.2)
        });
        assert!(common.score < same.score);
    }

    #[test]
    fn missing_values_never_help_under_conservative_handling() {
        let rules = rules();
        let query = person("Akello", "Grace", 1990, None);
        let mut sparse = query.clone();
        sparse.phone = None;
        sparse.district = None;
        let full = evaluate(&query, &query.clone(), &rules, &Consonants, &|_, _| None).score;
        let conservative = evaluate(&query, &sparse, &rules, &Consonants, &|_, _| None).score;
        let greedy = evaluate(
            &query,
            &sparse,
            &MatchingRules {
                null_handling: NullHandling::Greedy,
                ..rules.clone()
            },
            &Consonants,
            &|_, _| None,
        )
        .score;
        let moderate = evaluate(
            &query,
            &sparse,
            &MatchingRules {
                null_handling: NullHandling::Moderate,
                ..rules
            },
            &Consonants,
            &|_, _| None,
        )
        .score;
        assert!(conservative < moderate && moderate < greedy && greedy < full);
    }

    #[test]
    fn keys_follow_adr_0055() {
        let rules = rules();
        let p = person("Akello", "Grace", 1990, Some(("CF90AAAA", false)));
        let stored = index_keys(&p, &Consonants, &rules);
        let kinds: BTreeSet<KeyKind> = stored.iter().map(|k| k.kind).collect();
        assert_eq!(kinds.len(), 5);
        assert!(stored.contains(&BlockKey {
            kind: KeyKind::Phone,
            value: "772123456".into()
        }));
        // fam_given_yob sorts the codes, so swapped names share it.
        let mut swapped = p.clone();
        swapped.family = Some("Grace".into());
        swapped.given = vec!["Akello".into()];
        let key = |q: &Person| {
            index_keys(q, &Consonants, &rules)
                .into_iter()
                .find(|k| k.kind == KeyKind::FamGivenYob)
        };
        assert_eq!(key(&p), key(&swapped));
        // Queries spread the year: ±1 on fam_given_yob only, ±5 everywhere when estimated.
        let query = query_keys(&p, &Consonants, &rules);
        assert_eq!(
            query
                .iter()
                .filter(|k| k.kind == KeyKind::FamGivenYob)
                .count(),
            3
        );
        assert_eq!(
            query
                .iter()
                .filter(|k| k.kind == KeyKind::FamYobDist)
                .count(),
            1
        );
        let estimated = Person {
            birth_estimated: true,
            ..p.clone()
        };
        assert_eq!(
            query_keys(&estimated, &Consonants, &rules)
                .iter()
                .filter(|k| k.kind == KeyKind::FamYobDist)
                .count(),
            11
        );
    }

    #[test]
    fn oversized_blocks_are_skipped_and_too_little_data_is_reported() {
        let rules = rules();
        let p = person("Akello", "Grace", 1990, None);
        let keys = query_keys(&p, &Consonants, &rules);
        let total = keys.len();
        let plan_ = plan(
            keys,
            &|k| if k.kind == KeyKind::Phone { 5_000 } else { 10 },
            &rules,
        );
        assert_eq!(plan_.skipped.len(), 1);
        assert_eq!(plan_.used.len(), total - 1);
        assert!(!plan_.insufficient());
        let nobody = Person {
            given: vec!["Grace".into()],
            ..Person::default()
        };
        assert!(plan(query_keys(&nobody, &Consonants, &rules), &|_| 0, &rules).insufficient());
        assert_eq!(MatchGrade::Possible.code(), "possible");
    }
}
