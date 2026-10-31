//! Matching quality on synthetic Ugandan data (ADR 0055; testing.md § Data):
//!
//! - the **CI corruption benchmark**: corrupted duplicates (typos, swapped names, ages off by up
//!   to five years, missing NIN, changed phones) must keep **blocking recall ≥ 99%** with
//!   **candidate sets under 2,000**, reporting the share of keys the stop-list skipped;
//! - the **precision/recall report** from labelled pairs at the auto-link and review
//!   thresholds.
//!
//! The population is far smaller than 45M; block sizes at national scale are the load test's.

use std::collections::{BTreeMap, HashMap, HashSet};

use health_client_registry::matching::{
    BlockKey, MatchGrade, MatchingRules, Person, evaluate, index_keys, plan, query_keys,
};
use health_registries_app::fhir::RegistryConfig;
use health_registries_app::matching::{DoubleMetaphone, person};
use synthetic_data::population::{Population, systems};
use synthetic_data::rng::Rng;
use synthetic_data::world::World;

const POPULATION: usize = 60_000;
const DUPLICATES: usize = 2_000;

fn rules() -> MatchingRules {
    MatchingRules::platform_default(systems::NIN, &[systems::NIN, "urn:health:ecid"])
}

/// One typo: a substitution, deletion, insertion or adjacent transposition.
fn typo(rng: &mut Rng, word: &str) -> String {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.len() < 3 {
        return word.to_owned();
    }
    let i =
        1 + usize::try_from(rng.below(u64::try_from(chars.len() - 2).unwrap_or(1))).unwrap_or(0);
    match rng.below(4) {
        0 => chars[i] = *rng.pick(&['a', 'e', 'i', 'o', 'u', 'k', 'n', 'm']),
        1 => {
            chars.remove(i);
        }
        2 => chars.insert(i, chars[i]),
        _ => chars.swap(i, i - 1),
    }
    chars.into_iter().collect()
}

/// A re-registration of `original` at another time: one or two corruptions.
fn corrupt(rng: &mut Rng, original: &Person) -> Person {
    let mut p = original.clone();
    p.id = None;
    let mut applied = 0;
    while applied == 0 || (applied < 2 && rng.chance(0.4)) {
        match rng.below(6) {
            0 => p.family = p.family.as_deref().map(|f| typo(rng, f)),
            1 => p.given = p.given.iter().map(|g| typo(rng, g)).collect(),
            2 => {
                // Names recorded the other way round.
                if let (Some(family), Some(given)) = (p.family.clone(), p.given.first().cloned()) {
                    p.family = Some(given);
                    p.given = vec![family];
                }
            }
            3 => {
                // An age that is out by up to five years is recorded as an estimate.
                let shift = i32::try_from(rng.between(1, 5)).unwrap_or(1)
                    * if rng.chance(0.5) { 1 } else { -1 };
                p.birth_year = p.birth_year.map(|y| y + shift);
                if shift.abs() > 1 {
                    p.birth_estimated = true;
                }
            }
            4 => p.identifiers.clear(),
            _ => p.phone = Some(format!("+2567{}", 70_000_000 + rng.below(9_999_999))),
        }
        applied += 1;
    }
    p
}

struct Index {
    people: Vec<Person>,
    blocks: HashMap<BlockKey, Vec<usize>>,
    frequency: HashMap<(u8, String), usize>,
}

impl Index {
    fn build(people: Vec<Person>, rules: &MatchingRules) -> Self {
        let mut blocks: HashMap<BlockKey, Vec<usize>> = HashMap::new();
        let mut frequency: HashMap<(u8, String), usize> = HashMap::new();
        for (i, p) in people.iter().enumerate() {
            for key in index_keys(p, &DoubleMetaphone, rules) {
                blocks.entry(key).or_default().push(i);
            }
            if let Some(f) = &p.family {
                *frequency
                    .entry((0, health_client_registry::matching::fold(f)))
                    .or_default() += 1;
            }
            if let Some(phone) = &p.phone {
                *frequency
                    .entry((1, health_client_registry::matching::phone_key(phone)))
                    .or_default() += 1;
            }
        }
        Self {
            people,
            blocks,
            frequency,
        }
    }
}

#[test]
fn blocking_recall_and_match_precision_on_corrupted_duplicates() {
    let rules = rules();
    let config = RegistryConfig {
        nin_system: systems::NIN.into(),
    };
    let world = World::new(11);
    let people: Vec<Person> = Population::new(11, &world, 4)
        .take(POPULATION)
        .enumerate()
        .map(|(i, json)| {
            let mut p = person(&json, &config);
            p.id = Some(i.to_string());
            p
        })
        .collect();
    let index = Index::build(people, &rules);
    #[allow(clippy::cast_precision_loss)]
    let total = POPULATION as f64;
    let frequency = |field: health_client_registry::matching::MatchField, value: &str| {
        let kind = match field {
            health_client_registry::matching::MatchField::Family => 0,
            health_client_registry::matching::MatchField::Phone => 1,
            _ => return None,
        };
        #[allow(clippy::cast_precision_loss)]
        index
            .frequency
            .get(&(kind, value.to_owned()))
            .map(|n| *n as f64 / total)
    };

    let mut rng = Rng::new(99, 0);
    let (mut blocked, mut skipped_keys, mut all_keys) = (0usize, 0usize, 0usize);
    let mut sizes = Vec::new();
    let mut grades: BTreeMap<&str, (usize, usize)> = BTreeMap::new(); // grade -> (true, false)
    let mut seen = HashSet::new();
    while seen.len() < DUPLICATES {
        let original = usize::try_from(rng.below(POPULATION as u64)).unwrap_or(0);
        if !seen.insert(original) {
            continue;
        }
        let query = corrupt(&mut rng, &index.people[original]);
        let keys = query_keys(&query, &DoubleMetaphone, &rules);
        all_keys += keys.len();
        let plan_ = plan(
            keys,
            &|k| index.blocks.get(k).map_or(0, |b| b.len() as u64),
            &rules,
        );
        skipped_keys += plan_.skipped.len();
        let mut candidates: HashSet<usize> = HashSet::new();
        for key in &plan_.used {
            candidates.extend(index.blocks.get(key).into_iter().flatten());
        }
        sizes.push(candidates.len());
        if candidates.contains(&original) {
            blocked += 1;
        }
        for c in candidates {
            let grade = evaluate(
                &query,
                &index.people[c],
                &rules,
                &DoubleMetaphone,
                &frequency,
            )
            .grade;
            let slot = grades.entry(grade.code()).or_default();
            if c == original {
                slot.0 += 1
            } else {
                slot.1 += 1
            }
        }
    }

    sizes.sort_unstable();
    #[allow(clippy::cast_precision_loss)]
    let (recall, skipped_share) = (
        blocked as f64 / DUPLICATES as f64,
        skipped_keys as f64 / all_keys as f64,
    );
    let p95 = sizes[sizes.len() * 95 / 100];
    let max = sizes.last().copied().unwrap_or(0);
    let count = |g: MatchGrade| grades.get(g.code()).copied().unwrap_or((0, 0));
    let (auto_true, auto_false) = {
        let (a, b) = count(MatchGrade::Probable);
        let (c, d) = count(MatchGrade::Certain);
        (a + c, b + d)
    };
    let (review_true, review_false) = count(MatchGrade::Possible);
    #[allow(clippy::cast_precision_loss)]
    let precision = auto_true as f64 / (auto_true + auto_false).max(1) as f64;
    #[allow(clippy::cast_precision_loss)]
    let auto_recall = auto_true as f64 / DUPLICATES as f64;
    #[allow(clippy::cast_precision_loss)]
    let found = (auto_true + review_true) as f64 / DUPLICATES as f64;
    eprintln!(
        "matching quality ({POPULATION} people, {DUPLICATES} corrupted duplicates, rules {}):\n\
         blocking recall {:.2}%, candidates p95 {p95} max {max}, skipped keys {:.2}%\n\
         auto-link: {auto_true} true, {auto_false} false (precision {:.3}%, recall {:.1}%)\n\
         review: {review_true} true, {review_false} false; found at auto-link or review {:.1}%",
        rules.version,
        recall * 100.0,
        skipped_share * 100.0,
        precision * 100.0,
        auto_recall * 100.0,
        found * 100.0,
    );
    assert!(recall >= 0.99, "blocking recall {recall}");
    assert!(max < 2_000, "candidate set of {max}");
    assert!(precision >= 0.999, "auto-link precision {precision}");
}
