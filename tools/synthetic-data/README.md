# synthetic-data

Layer: **tooling**. Synthetic Ugandan patients, facilities, stores, stock, prices and payer contracts for CI, staging and load tests ([testing.md](../../docs/architecture/testing.md) § Data). Only synthetic data enters CI. Every resource is tagged `urn:health:synthetic-data` (code `synthetic`), and its identifiers use `urn:health:synthetic-data/…` systems.

```sh
cargo run --release -p synthetic-data -- generate --out DIR [--seed 1] [--patients 1000] [--health-worker-phones N]
cargo run --release -p synthetic-data -- load --dir DIR --base http://127.0.0.1:8080/fhir [--bearer TOKEN] [--batch 250] [--concurrency 8]
```

## Output

`generate` writes one NDJSON file per load step and a `manifest.json`. The manifest records the seed, the counts and each file's SHA-256, in load order: a file's references point only at earlier files.

| File | Resources |
|---|---|
| `Organization.ndjson` | About 275 facilities (`prov`), from one national referral hospital and a regional referral hospital per region down to HC II, about six per district in 45 districts. Also 4 payers (`pay`) |
| `Location.facility.ndjson` | One building per facility |
| `Location.store.ndjson` | Main store and pharmacy at every facility; a laboratory and a ward store at hospitals and HC IVs (`partOf` the building) |
| `Medication.ndjson` | 20 essential medicines |
| `InventoryItem.ndjson` | A stock balance, lot and expiry per store and product. Main stores and pharmacies hold every product; other stores hold a quarter of them |
| `ChargeItemDefinition.ndjson` | One price book (`2026.1`): 12 services and the medicines, in UGX |
| `InsurancePlan.ndjson`, `Contract.ndjson` | A plan per payer, and a contract with each hospital and HC IV in the regions it covers |
| `Patient.ndjson` | The patients, household by household |

- **Reproducible:** the same seed and size give byte-identical files. The PRNG is the crate's own xoshiro256** (`src/rng.rs`), because `rand` does not promise stable values across versions.
- **Ids:** every id is a UUIDv5 under the platform namespace, derived from the seed, the kind and the index (`health_identifiers::derive_id`).

## Patients

The distributions follow the matching-scale spike ([docs/research/patient-identity.md](../../docs/research/patient-identity.md) § Matching-scale spike) and identifiers.md:

- **Regions and districts:** the four regions weighted by population. Districts are Zipf-skewed within each region, and each household registers at a facility in its district.
- **Households:** size 1 + Poisson(4), so the mean is 5. A household has a head, a spouse with probability 0.65, and children. About a fifth of the other members are adult relatives. 1.5% of births are twins, with the same birth date.
- **Surnames:** one pool of 1,250 per region, Zipf–Mandelbrot. Each pool holds frequent real names first, then generated ones.
  - **Central (Buganda):** surnames come from the father's clan's share of the pool (52 clans), so siblings share a clan but rarely a surname. Ganda twins are named Waswa/Babirye and Kato/Nakato.
  - **Elsewhere:** a member takes the household surname with probability 0.6.
  - **Gender:** Luo names change their leading vowel by sex (Okello, Akello). Ganda female names often take `Na-` (Mukasa, Namukasa).
  - **The clan assignment is synthetic.** It models the effect on matching, not real clan membership.
- **Given names:** English or Christian names, and Muslim names in 14% of households.
- **Ages:** the median is in the mid-teens. 30% of adults have an estimated age, recorded as a birth year only and rounded to 5 years.
- **Phones:**
  - 70% of households share one phone.
  - Some adults have their own.
  - 2% of the rest give a health worker's phone, from a pool of 3,000 per 45M people.
  - Numbers use MTN and Airtel prefixes.
- **Identifiers:**
  - Every patient has an MRN from the registering facility, `MRN-{facility}-E1-{sequence}{check}` (ADR 0014 format, Luhn mod 36).
  - A NIN-shaped value is given to 90% of adults and 55% of children, about 75% overall. 0.1% of NINs repeat another person's.

## Loading

`load` sends batch Bundles of `PUT {type}/{id}` (update-as-create) and fails on the first entry the server refuses.

- **Order:** files load in manifest order, one at a time, because references point only at earlier files.
- **Plain HTTP only.** Every TLS option of the HTTP client (reqwest) brings `webpki-root-certs`, which is CDLA-Permissive-2.0, a licence ADR 0045 does not allow.
- **Loading twice:** a second load into the same database is refused (428), because updates need `If-Match`. It cannot duplicate anything, so load into an empty database.

## Tests

- **Unit:** the PRNG, the pools and the patient distributions.
- **`tests/load.rs`:**
  - Generates twice and compares the manifests.
  - Loads a small population through a real `FhirServer` on the SurrealDB memory engine, so tiers 1 and 2 validate every resource.
  - Checks that a second load is refused and that the read-back matches.

## Measured (2026-10-31, laptop: macOS x86_64 i9, 32 GB; release builds)

| Step | Result |
|---|---|
| `generate --seed 1 --patients 1000000` | 32–39 s, 110 MB peak memory, 810 MB of NDJSON. Two runs gave byte-identical files (Patient.ndjson SHA-256 `a4334c14…34d2208`) |
| `load` into `health-fhir-server`'s `contract_server` (memory engine, Patient search parameters indexed), batch 250, concurrency 8 | All 1,013,507 resources accepted, none refused. Patients: 1,000,000 in 27.4 min (about 610 per second). The server's peak memory was 12.9 GB |
