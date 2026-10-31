//! Facilities, stores, products, stock, prices and payers: the world the patients live in.

use health_identifiers::{DerivationName, derive_id};
use serde_json::{Value, json};

use crate::names::{Region, village};
use crate::rng::Rng;

/// Marks every generated resource, so synthetic data is never mistaken for real data.
pub const SYNTHETIC_TAG: &str = "urn:health:synthetic-data";

/// The deterministic id of the `index`th `kind` for `seed`: a UUIDv5 under the platform
/// namespace (identifiers.md), so reloading the same output updates rather than duplicates.
pub fn id(seed: u64, kind: &str, index: usize) -> String {
    derive_id(&DerivationName::rule(
        format!("{seed}/{kind}/{index}"),
        SYNTHETIC_TAG,
    ))
    .to_string()
}

/// The `meta` every generated resource carries.
pub fn meta() -> Value {
    json!({ "tag": [{ "system": SYNTHETIC_TAG, "code": "synthetic", "display": "Synthetic data" }] })
}

/// The level of a facility in the Ugandan health system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// National referral hospital.
    National,
    /// Regional referral hospital.
    Regional,
    /// General (district) hospital.
    General,
    /// Health Centre IV (county).
    Hc4,
    /// Health Centre III (sub-county).
    Hc3,
    /// Health Centre II (parish).
    Hc2,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::National => "National Referral Hospital",
            Self::Regional => "Regional Referral Hospital",
            Self::General => "General Hospital",
            Self::Hc4 => "Health Centre IV",
            Self::Hc3 => "Health Centre III",
            Self::Hc2 => "Health Centre II",
        }
    }

    /// Hospitals and HC IVs have a laboratory and wards, and contract with payers.
    pub fn is_hospital_or_hc4(self) -> bool {
        !matches!(self, Self::Hc3 | Self::Hc2)
    }
}

/// One facility: an Organization and its physical Location.
#[derive(Debug, Clone)]
pub struct Facility {
    /// The Organization id.
    pub organization: String,
    /// The building Location id.
    pub location: String,
    /// The facility code used in its MRNs (`A–Z0–9`, at most 8).
    pub code: String,
    /// The facility's level.
    pub level: Level,
    /// The region it serves.
    pub region: Region,
    /// The district it is in.
    pub district: &'static str,
}

/// A payer: an insurer or a community scheme.
#[derive(Debug, Clone)]
pub struct Payer {
    /// The Organization id.
    pub organization: String,
    /// The regions it covers.
    pub regions: Vec<Region>,
}

/// Every non-patient resource, grouped as the loader must order them.
#[derive(Debug, Default)]
pub struct World {
    /// The facilities, by region and district.
    pub facilities: Vec<Facility>,
    /// The payers.
    pub payers: Vec<Payer>,
    /// Organizations: facilities, then payers.
    pub organizations: Vec<Value>,
    /// Facility buildings.
    pub facility_locations: Vec<Value>,
    /// Stores and service points, `partOf` a facility building.
    pub store_locations: Vec<Value>,
    /// Products.
    pub medications: Vec<Value>,
    /// Stock balances per store and product.
    pub inventory: Vec<Value>,
    /// The price book's entries.
    pub prices: Vec<Value>,
    /// Benefit plans.
    pub plans: Vec<Value>,
    /// Payer contracts with facilities.
    pub contracts: Vec<Value>,
}

/// Essential medicines: (name, dose form, unit).
const PRODUCTS: &[(&str, &str, &str)] = &[
    ("Amoxicillin 500 mg", "Capsule", "capsule"),
    ("Artemether/Lumefantrine 20/120 mg", "Tablet", "tablet"),
    ("Paracetamol 500 mg", "Tablet", "tablet"),
    (
        "Oral rehydration salts",
        "Powder for oral solution",
        "sachet",
    ),
    ("Zinc sulfate 20 mg", "Dispersible tablet", "tablet"),
    ("Cotrimoxazole 960 mg", "Tablet", "tablet"),
    (
        "Tenofovir/Lamivudine/Dolutegravir 300/300/50 mg",
        "Tablet",
        "tablet",
    ),
    (
        "Rifampicin/Isoniazid/Pyrazinamide/Ethambutol 150/75/400/275 mg",
        "Tablet",
        "tablet",
    ),
    ("Metformin 500 mg", "Tablet", "tablet"),
    ("Amlodipine 5 mg", "Tablet", "tablet"),
    ("Ferrous sulfate/Folic acid 200/0.4 mg", "Tablet", "tablet"),
    ("Oxytocin 10 IU/mL", "Injection", "ampoule"),
    ("Ceftriaxone 1 g", "Powder for injection", "vial"),
    ("Metronidazole 400 mg", "Tablet", "tablet"),
    ("Albendazole 400 mg", "Chewable tablet", "tablet"),
    ("Medroxyprogesterone acetate 150 mg/mL", "Injection", "vial"),
    ("Malaria rapid diagnostic test", "Test kit", "test"),
    ("Sodium chloride 0.9% 500 mL", "Infusion", "bottle"),
    ("Ibuprofen 200 mg", "Tablet", "tablet"),
    ("Salbutamol 100 mcg/dose", "Inhaler", "inhaler"),
];

/// Billable services: (name, UGX price at a private wing).
const SERVICES: &[(&str, u32)] = &[
    ("General outpatient consultation", 10_000),
    ("Specialist consultation", 30_000),
    ("Malaria blood smear", 5_000),
    ("Full blood count", 15_000),
    ("Urinalysis", 5_000),
    ("HIV rapid test", 0),
    ("Obstetric ultrasound", 40_000),
    ("Chest X-ray", 35_000),
    ("Normal delivery", 150_000),
    ("Inpatient bed day (general ward)", 20_000),
    ("Wound dressing", 5_000),
    ("Dental extraction", 20_000),
];

/// Insurers and schemes: (name, regions covered; empty means national).
const PAYERS: &[(&str, &[Region])] = &[
    ("Synthetic National Health Insurance", &[]),
    (
        "Synthetic Mutual Assurance",
        &[Region::Central, Region::Western],
    ),
    (
        "Synthetic Eastern Community Health Scheme",
        &[Region::Eastern],
    ),
    (
        "Synthetic Northern Community Health Scheme",
        &[Region::Northern],
    ),
];

impl World {
    /// The world for `seed`: about six facilities per district, each with its stores and stock,
    /// one price book, and the payers with their plans and contracts.
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed, 0x574f_524c);
        let mut world = Self::default();
        for (region, _) in Region::ALL {
            for (rank, &district) in region.districts().iter().enumerate() {
                let mut levels = vec![
                    Level::General,
                    Level::Hc4,
                    Level::Hc3,
                    Level::Hc3,
                    Level::Hc2,
                    Level::Hc2,
                ];
                if rank == 0 {
                    levels.insert(0, Level::Regional);
                }
                if district == "Kampala" {
                    levels.insert(0, Level::National);
                }
                for level in levels {
                    world.add_facility(seed, &mut rng, region, district, level);
                }
            }
        }
        world.add_products(seed, &mut rng);
        world.add_prices(seed);
        world.add_payers(seed);
        world
    }

    fn add_facility(
        &mut self,
        seed: u64,
        rng: &mut Rng,
        region: Region,
        district: &'static str,
        level: Level,
    ) {
        let index = self.facilities.len();
        let place = match level {
            Level::National | Level::Regional | Level::General => district.to_owned(),
            Level::Hc4 | Level::Hc3 | Level::Hc2 => village(rng),
        };
        let name = format!("{place} {}", level.label());
        let code = format!("{}{index:03}", district[..3].to_uppercase());
        let facility = Facility {
            organization: id(seed, "facility-organization", index),
            location: id(seed, "facility-location", index),
            code: code.clone(),
            level,
            region,
            district,
        };
        let address = json!({ "district": district, "state": region.name(), "country": "UG" });
        self.organizations.push(json!({
            "resourceType": "Organization",
            "id": facility.organization,
            "meta": meta(),
            "identifier": [{ "system": format!("{SYNTHETIC_TAG}/facility-code"), "value": code }],
            "active": true,
            "type": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/organization-type", "code": "prov", "display": "Healthcare Provider" }] }],
            "name": name,
            "contact": [{ "address": address }],
        }));
        self.facility_locations.push(json!({
            "resourceType": "Location",
            "id": facility.location,
            "meta": meta(),
            "status": "active",
            "name": name,
            "mode": "instance",
            "address": address,
            "form": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/location-physical-type", "code": "bu", "display": "Building" }] },
            "managingOrganization": { "reference": format!("Organization/{}", facility.organization) },
        }));
        let mut stores = vec![
            ("Main store", None),
            ("Pharmacy", Some(("PHARM", "Pharmacy"))),
        ];
        if level.is_hospital_or_hc4() {
            stores.push(("Laboratory", Some(("HLAB", "hospital laboratory"))));
            stores.push(("Ward store", None));
        }
        for (store, role) in stores {
            let index = self.store_locations.len();
            let mut location = json!({
                "resourceType": "Location",
                "id": id(seed, "store-location", index),
                "meta": meta(),
                "status": "active",
                "name": format!("{name} — {store}"),
                "mode": "instance",
                "form": { "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/location-physical-type", "code": "ro", "display": "Room" }] },
                "partOf": { "reference": format!("Location/{}", facility.location) },
                "managingOrganization": { "reference": format!("Organization/{}", facility.organization) },
            });
            if let Some((code, display)) = role {
                location["type"] = json!([{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/v3-RoleCode", "code": code, "display": display }] }]);
            }
            self.store_locations.push(location);
        }
        self.facilities.push(facility);
    }

    fn add_products(&mut self, seed: u64, rng: &mut Rng) {
        for (index, &(name, form, _)) in PRODUCTS.iter().enumerate() {
            self.medications.push(json!({
                "resourceType": "Medication",
                "id": id(seed, "medication", index),
                "meta": meta(),
                "code": { "text": name },
                "status": "active",
                "doseForm": { "text": form },
            }));
        }
        // Main stores and pharmacies hold every product; laboratories and ward stores a few.
        let stores: Vec<(String, bool)> = self
            .store_locations
            .iter()
            .map(|store| {
                let name = store["name"].as_str().unwrap_or_default();
                (
                    store["id"].as_str().unwrap_or_default().to_owned(),
                    name.ends_with("Main store") || name.ends_with("Pharmacy"),
                )
            })
            .collect();
        for (store, full) in stores {
            for (product, &(name, _, unit)) in PRODUCTS.iter().enumerate() {
                if !full && !rng.chance(0.25) {
                    continue;
                }
                let index = self.inventory.len();
                let balance = rng.between(0, 5_000);
                let expiry = format!("{}-{:02}-28", rng.between(2027, 2029), rng.between(1, 12));
                self.inventory.push(json!({
                    "resourceType": "InventoryItem",
                    "id": id(seed, "inventory-item", index),
                    "meta": meta(),
                    "status": "active",
                    "code": [{ "text": name }],
                    "netContent": { "value": balance, "unit": unit },
                    "instance": {
                        "lotNumber": format!("L{:06}", rng.below(1_000_000)),
                        "expiry": expiry,
                        "location": { "reference": format!("Location/{store}") },
                    },
                    "productReference": { "reference": format!("Medication/{}", id(seed, "medication", product)) },
                }));
            }
        }
    }

    fn add_prices(&mut self, seed: u64) {
        let services = SERVICES.iter().map(|&(name, price)| (name, price));
        // Medicines are priced per unit; the amounts are illustrative.
        let medicines = PRODUCTS
            .iter()
            .enumerate()
            .map(|(rank, &(name, _, _))| (name, 200 + 150 * u32::try_from(rank).unwrap_or(0)));
        for (index, (name, price)) in services.chain(medicines).enumerate() {
            self.prices.push(json!({
                "resourceType": "ChargeItemDefinition",
                "id": id(seed, "charge-item-definition", index),
                "meta": meta(),
                "url": format!("{SYNTHETIC_TAG}/price-book/private-wing/{index}"),
                "version": "2026.1",
                "name": format!("PrivateWing{index}"),
                "title": name,
                "status": "active",
                "code": { "text": name },
                "propertyGroup": [{ "priceComponent": [{ "type": "base", "amount": { "value": price, "currency": "UGX" } }] }],
            }));
        }
    }

    fn add_payers(&mut self, seed: u64) {
        for (index, &(name, regions)) in PAYERS.iter().enumerate() {
            let regions = if regions.is_empty() {
                Region::ALL.map(|(r, _)| r).to_vec()
            } else {
                regions.to_vec()
            };
            let payer = Payer {
                organization: id(seed, "payer-organization", index),
                regions,
            };
            self.organizations.push(json!({
                "resourceType": "Organization",
                "id": payer.organization,
                "meta": meta(),
                "active": true,
                "type": [{ "coding": [{ "system": "http://terminology.hl7.org/CodeSystem/organization-type", "code": "pay", "display": "Payer" }] }],
                "name": name,
            }));
            let network: Vec<&Facility> = self
                .facilities
                .iter()
                .filter(|f| f.level.is_hospital_or_hc4() && payer.regions.contains(&f.region))
                .collect();
            self.plans.push(json!({
                "resourceType": "InsurancePlan",
                "id": id(seed, "insurance-plan", index),
                "meta": meta(),
                "status": "active",
                "name": format!("{name} — standard cover"),
                "ownedBy": { "reference": format!("Organization/{}", payer.organization) },
                "network": network.iter().map(|f| json!({ "reference": format!("Organization/{}", f.organization) })).collect::<Vec<_>>(),
                "plan": [{
                    "type": { "text": "Standard" },
                    "generalCost": [{ "type": { "text": "Outpatient copayment" }, "cost": { "value": 2_000, "currency": "UGX" } }],
                }],
            }));
            for facility in network {
                let index = self.contracts.len();
                self.contracts.push(json!({
                    "resourceType": "Contract",
                    "id": id(seed, "contract", index),
                    "meta": meta(),
                    "status": "executed",
                    "name": format!("{name} — {}", facility.code),
                    "applies": { "start": "2026-01-01", "end": "2026-12-31" },
                    "authority": [{ "reference": format!("Organization/{}", payer.organization) }],
                    "subject": [{ "reference": format!("Organization/{}", facility.organization) }],
                }));
            }
            self.payers.push(payer);
        }
    }
}
