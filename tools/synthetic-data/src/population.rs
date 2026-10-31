//! Patients, generated household by household (testing.md § Data; the distributions follow the
//! matching-scale spike in docs/research/patient-identity.md).

use std::collections::VecDeque;

use health_identifiers::{CheckDigit, NumberFormat, ShortCode};
use serde_json::{Value, json};

use crate::names::{
    GANDA_TWINS, GIVEN_FEMALE, GIVEN_FEMALE_MUSLIM, GIVEN_MALE, GIVEN_MALE_MUSLIM, Region,
    SurnamePool, village,
};
use crate::rng::{Rng, Weighted};
use crate::world::{World, id, meta};

/// The year ages are counted from.
pub const REFERENCE_YEAR: u32 = 2026;
/// Buganda's clans; each owns a share of the Central surname pool.
const CLANS: usize = 52;
/// The probability that a non-Central household member takes the household surname.
const SHARED_SURNAME: f64 = 0.6;
/// The probability that a household has a phone, shared by every member.
const HOUSEHOLD_PHONE: f64 = 0.7;
/// NIN coverage, which is about 75% overall (identifiers.md), split by age.
const NIN_ADULT: f64 = 0.9;
const NIN_CHILD: f64 = 0.55;
/// NINs entered twice in error.
const NIN_DUPLICATE: f64 = 0.001;
/// Adults whose age is an estimate, recorded as a birth year that is a multiple of 5 years ago.
const ESTIMATED_AGE: f64 = 0.3;
/// Births that are twins.
const TWINS: f64 = 0.015;
/// Muslim households (given names).
const MUSLIM: f64 = 0.14;

/// The identifier systems the generated patients use.
pub mod systems {
    /// National Identification Number (synthetic).
    pub const NIN: &str = "urn:health:synthetic-data/nin";
    /// The registering facility's medical record number.
    pub const MRN: &str = "urn:health:synthetic-data/mrn";
}

struct Member {
    female: bool,
    age: u32,
    estimated: bool,
    surname: String,
    birth: Option<(u32, u32)>,
}

/// An endless, reproducible stream of Patients for one seed and world.
pub struct Population<'w> {
    seed: u64,
    rng: Rng,
    world: &'w World,
    pools: Vec<SurnamePool>,
    regions: Weighted,
    districts: Vec<Weighted>,
    clans: Weighted,
    health_worker_phones: Vec<String>,
    mrn: NumberFormat,
    node: ShortCode,
    sequences: Vec<u64>,
    issued_nins: Vec<String>,
    pending: VecDeque<Value>,
    next: usize,
}

impl<'w> Population<'w> {
    /// The stream for `seed`. `health_worker_phones` is the pool of phones that health workers
    /// lend to patients without one (3,000 in the spike at 45M).
    pub fn new(seed: u64, world: &'w World, health_worker_phones: usize) -> Self {
        let mut rng = Rng::new(seed, 0x504f_5055);
        let pools = Region::ALL
            .iter()
            .map(|&(region, _)| SurnamePool::new(region, seed))
            .collect();
        let phones = (0..health_worker_phones).map(|_| phone(&mut rng)).collect();
        Self {
            seed,
            rng,
            world,
            pools,
            regions: Weighted::new(Region::ALL.map(|(_, share)| share)),
            districts: Region::ALL
                .iter()
                .map(|(region, _)| Weighted::zipf(region.districts().len(), 1.0, 1.0))
                .collect(),
            clans: Weighted::zipf(CLANS, 0.8, 3.0),
            health_worker_phones: phones,
            mrn: NumberFormat {
                prefix: "MRN".parse().ok(),
                padding: 6,
                check_digit: Some(CheckDigit::LuhnMod36),
                yearly_reset: false,
            },
            node: "E1"
                .parse()
                .unwrap_or_else(|_| unreachable!("E1 is a valid short code")),
            sequences: vec![0; world.facilities.len()],
            issued_nins: Vec::new(),
            pending: VecDeque::new(),
            next: 0,
        }
    }

    fn household(&mut self) {
        let region_index = self.regions.sample(&mut self.rng);
        let region = Region::ALL[region_index].0;
        let district = region.districts()[self.districts[region_index].sample(&mut self.rng)];
        let facilities: Vec<usize> = (0..self.world.facilities.len())
            .filter(|&i| self.world.facilities[i].district == district)
            .collect();
        let facility = *self.rng.pick(&facilities);
        let place = village(&mut self.rng);
        let household_phone = self
            .rng
            .chance(HOUSEHOLD_PHONE)
            .then(|| phone(&mut self.rng));
        let muslim = self.rng.chance(MUSLIM);

        for member in self.members(region_index) {
            let index = self.next;
            self.next += 1;
            let given = match (member.female, muslim) {
                (false, false) => self.rng.pick(GIVEN_MALE),
                (true, false) => self.rng.pick(GIVEN_FEMALE),
                (false, true) => self.rng.pick(GIVEN_MALE_MUSLIM),
                (true, true) => self.rng.pick(GIVEN_FEMALE_MUSLIM),
            };
            let adult = member.age >= 16;
            let phone = household_phone.clone().or_else(|| {
                if adult && self.rng.chance(0.2) {
                    Some(phone(&mut self.rng))
                } else if !self.health_worker_phones.is_empty() && self.rng.chance(0.02) {
                    Some(self.rng.pick(&self.health_worker_phones).clone())
                } else {
                    None
                }
            });
            let mut identifier = vec![self.mrn_for(facility)];
            if self.rng.chance(if adult { NIN_ADULT } else { NIN_CHILD }) {
                identifier.push(json!({ "system": systems::NIN, "value": self.nin(&member) }));
            }
            let birth_date = match (member.estimated, member.birth) {
                (true, _) | (false, None) => (REFERENCE_YEAR - member.age).to_string(),
                (false, Some((month, day))) => {
                    format!("{}-{month:02}-{day:02}", REFERENCE_YEAR - member.age)
                }
            };
            let mut patient = json!({
                "resourceType": "Patient",
                "id": id(self.seed, "patient", index),
                "meta": meta(),
                "identifier": identifier,
                "active": true,
                "name": [{ "use": "official", "family": member.surname, "given": [given] }],
                "gender": if member.female { "female" } else { "male" },
                "birthDate": birth_date,
                "address": [{ "city": place, "district": district, "state": region.name(), "country": "UG" }],
                "managingOrganization": { "reference": format!("Organization/{}", self.world.facilities[facility].organization) },
            });
            if let Some(phone) = phone {
                patient["telecom"] =
                    json!([{ "system": "phone", "value": phone, "use": "mobile" }]);
            }
            self.pending.push_back(patient);
        }
    }

    /// A household: a head, often a spouse, and children (mean size 5).
    fn members(&mut self, region_index: usize) -> Vec<Member> {
        let region = Region::ALL[region_index].0;
        let size = (1 + self.rng.poisson(4.0)).min(12);
        let head_female = self.rng.chance(0.3);
        let head_age = (22.0 + self.rng.exponential(16.0)).min(85.0);
        // Truncation to whole years is intended.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let head_age = head_age as u32;
        let clan = self.clans.sample(&mut self.rng);
        let household_rank = self.pools[region_index].rank(&mut self.rng);

        let mut members = Vec::new();
        let head_surname = self.surname(region_index, clan, household_rank, head_female, true);
        members.push(self.member(head_female, head_age, head_surname));
        if size >= 2 && self.rng.chance(0.65) {
            let age = head_age.saturating_sub(self.rng.between(0, 8)).max(18);
            let other_clan =
                (clan + 1 + usize::try_from(self.rng.between(0, 5)).unwrap_or(0)) % CLANS;
            let surname = self.surname(
                region_index,
                other_clan,
                household_rank,
                !head_female,
                false,
            );
            members.push(self.member(!head_female, age, surname));
        }
        let oldest_child = head_age.saturating_sub(16).min(35);
        while members.len() < usize::try_from(size).unwrap_or(1) {
            let female = self.rng.chance(0.51);
            // Children and other dependants, spread over the head's child-bearing years.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let age = if self.rng.chance(0.2) {
                // An adult relative: a grown child, a sibling or a grandparent.
                (16.0 + self.rng.exponential(18.0)).min(90.0) as u32
            } else {
                (self.rng.unit() * f64::from(oldest_child + 1)) as u32
            };
            let surname = self.surname(region_index, clan, household_rank, female, false);
            let child = self.member(female, age, surname);
            let twins =
                members.len() + 1 < usize::try_from(size).unwrap_or(1) && self.rng.chance(TWINS);
            if twins {
                for (male, woman) in GANDA_TWINS {
                    let female = self.rng.chance(0.51);
                    let surname = self.surname(region_index, clan, household_rank, female, false);
                    let mut twin = Member {
                        female,
                        surname,
                        ..copy(&child)
                    };
                    if region == Region::Central {
                        twin.surname = (if female { woman } else { male }).to_owned();
                    }
                    members.push(twin);
                }
            } else {
                members.push(child);
            }
        }
        members.truncate(usize::try_from(size).unwrap_or(1));
        members
    }

    fn member(&mut self, female: bool, age: u32, surname: String) -> Member {
        let estimated = age >= 18 && self.rng.chance(ESTIMATED_AGE);
        let age = if estimated { (age + 2) / 5 * 5 } else { age };
        let birth = Some((self.rng.between(1, 12), self.rng.between(1, 28)));
        Member {
            female,
            age,
            estimated,
            surname,
            birth,
        }
    }

    /// Central surnames come from the clan's share of the pool; elsewhere a member takes the
    /// household's surname with probability [`SHARED_SURNAME`] (the head always does).
    fn surname(
        &mut self,
        region_index: usize,
        clan: usize,
        household_rank: usize,
        female: bool,
        head: bool,
    ) -> String {
        let pool = &self.pools[region_index];
        let rank = if pool.region() == Region::Central {
            let rank = pool.rank(&mut self.rng);
            rank - rank % CLANS + clan
        } else if head || self.rng.chance(SHARED_SURNAME) {
            household_rank
        } else {
            pool.rank(&mut self.rng)
        };
        pool.name(rank, female).to_owned()
    }

    fn mrn_for(&mut self, facility: usize) -> Value {
        self.sequences[facility] += 1;
        let code: ShortCode = self.world.facilities[facility]
            .code
            .parse()
            .unwrap_or_else(|_| unreachable!("facility codes are valid short codes"));
        let value = self.mrn.format(
            &code,
            &self.node,
            i32::try_from(REFERENCE_YEAR).unwrap_or(2026),
            self.sequences[facility],
        );
        json!({
            "use": "usual",
            "type": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v2-0203", "code": "MR" }] },
            "system": systems::MRN,
            "value": value,
            "assigner": { "reference": format!("Organization/{}", self.world.facilities[facility].organization) },
        })
    }

    /// A NIN-shaped value: `C`, sex, two digits of the birth year, ten characters. A small share
    /// repeats an earlier person's NIN, as entry errors do.
    fn nin(&mut self, member: &Member) -> String {
        if !self.issued_nins.is_empty() && self.rng.chance(NIN_DUPLICATE) {
            return self.rng.pick(&self.issued_nins).clone();
        }
        const CHARS: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
        let year = (REFERENCE_YEAR - member.age) % 100;
        let tail: String = (0..10).map(|_| char::from(*self.rng.pick(CHARS))).collect();
        let nin = format!("C{}{year:02}{tail}", if member.female { 'F' } else { 'M' });
        self.issued_nins.push(nin.clone());
        nin
    }
}

fn copy(member: &Member) -> Member {
    Member {
        surname: member.surname.clone(),
        ..*member
    }
}

/// A Ugandan mobile number (MTN or Airtel prefixes), in E.164.
fn phone(rng: &mut Rng) -> String {
    format!(
        "+2567{}{:07}",
        rng.pick(&[0, 4, 5, 6, 7, 8]),
        rng.below(10_000_000)
    )
}

impl Iterator for Population<'_> {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        while self.pending.is_empty() {
            self.household();
        }
        self.pending.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::world::SYNTHETIC_TAG;

    fn sample(n: usize) -> Vec<Value> {
        let world = World::new(9);
        Population::new(9, &world, 30).take(n).collect()
    }

    #[test]
    fn the_distributions_hold() {
        let patients = sample(20_000);
        let share = |f: &dyn Fn(&Value) -> bool| {
            #[allow(clippy::cast_precision_loss)]
            let s = patients.iter().filter(|p| f(p)).count() as f64 / patients.len() as f64;
            s
        };
        let with_phone = share(&|p| p.get("telecom").is_some());
        assert!((0.68..0.85).contains(&with_phone), "phone {with_phone}");
        let with_nin = share(&|p| p["identifier"].as_array().is_some_and(|ids| ids.len() == 2));
        assert!((0.68..0.82).contains(&with_nin), "NIN {with_nin}");
        let mut ages: Vec<u32> = patients
            .iter()
            .filter_map(|p| p["birthDate"].as_str()?.get(..4)?.parse::<u32>().ok())
            .map(|year| REFERENCE_YEAR - year)
            .collect();
        ages.sort_unstable();
        let median = ages[ages.len() / 2];
        assert!((14..=20).contains(&median), "median age {median}");
        let female = share(&|p| p["gender"] == "female");
        assert!((0.45..0.55).contains(&female), "female {female}");
    }

    #[test]
    fn frequent_surnames_repeat_and_luo_names_follow_sex() {
        let patients = sample(20_000);
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for patient in &patients {
            *counts
                .entry(patient["name"][0]["family"].as_str().unwrap_or_default())
                .or_default() += 1;
            if patient["address"][0]["state"] == "Northern" {
                let family = patient["name"][0]["family"].as_str().unwrap_or_default();
                if family.starts_with('O') {
                    assert_eq!(patient["gender"], "male", "{family}");
                }
            }
        }
        let top = counts.values().max().copied().unwrap_or(0);
        assert!(top > 100, "the most frequent surname appears {top} times");
    }

    #[test]
    fn central_siblings_share_a_clan_not_a_surname() {
        // Over many Central households, children rarely share their head's surname.
        let patients = sample(20_000);
        let central: Vec<&Value> = patients
            .iter()
            .filter(|p| p["address"][0]["state"] == "Central")
            .collect();
        let pairs = central
            .windows(2)
            .filter(|w| w[0]["address"][0]["city"] == w[1]["address"][0]["city"]);
        let (same, total) = pairs.fold((0, 0), |(same, total), w| {
            (
                same + usize::from(w[0]["name"][0]["family"] == w[1]["name"][0]["family"]),
                total + 1,
            )
        });
        assert!(total > 100);
        assert!(
            same * 4 < total,
            "{same} of {total} neighbours share a surname"
        );
    }

    #[test]
    fn mrns_are_valid_numbers_unique_per_facility() {
        let patients = sample(5_000);
        let mut seen = std::collections::HashSet::new();
        for patient in &patients {
            let mrn = patient["identifier"][0]["value"]
                .as_str()
                .unwrap_or_default();
            assert!(mrn.starts_with("MRN-") && mrn.contains("-E1-"), "{mrn}");
            assert!(seen.insert(mrn.to_owned()), "duplicate {mrn}");
        }
    }

    #[test]
    fn every_patient_is_tagged_synthetic() {
        for patient in sample(100) {
            assert_eq!(patient["meta"]["tag"][0]["system"], SYNTHETIC_TAG);
        }
    }
}
