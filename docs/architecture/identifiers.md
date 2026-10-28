# Identifiers

Status: **accepted** (ADRs 0013, 0014). Produced while resolving the Wayfinder ticket [How are global and facility-local identifiers minted?](https://github.com/carapai/people-centered-healthcare/issues/17). Related decisions: [write-authority.md](write-authority.md) (Facility Authority Node), [lan-security.md](lan-security.md) (device enrollment), [offline-auth.md](offline-auth.md). Patient matching on these identifiers is decided in [How do patient matching, self-registration and caregiver delegation work?](https://github.com/carapai/people-centered-healthcare/issues/26).

## Kinds of identifier

| Kind | Format | Minted by | Shown to users |
|---|---|---|---|
| Resource / record id (`Resource.id`, internal records) | UUIDv7, lowercase hyphenated | The node that creates the record | No |
| Derived record id | UUIDv5 under the platform namespace | Any node running the derivation (they all compute the same id) | No |
| `node_id` | UUIDv7 | Central, at node provisioning | Admin screens only |
| Node short code | Short code unique within the facility (e.g. `E1`, `D07`) | Central, at provisioning | Embedded in human-readable numbers |
| `device_id` | UUIDv7, bound to the device key's JWK thumbprint (RFC 7638) | The edge, at enrollment | Admin screens only |
| `op_id` (sync operation) | UUIDv7 | The writing node, in the same transaction as the change | No |
| Business identifiers | Per NamingSystem (below) | See below | Yes |

In Rust, every id is a typed newtype in `health-identifiers` (e.g. `PatientId`, `NodeId`, `OpId`). Bare `Uuid` or `String` ids never cross a crate API.

## Rules

1. **Client-generated.** Records are created offline with their final id. A create that is replayed is idempotent by id.
2. **Never recycled.** A re-provisioned machine gets a new `node_id`, and a re-enrolled device gets a new `device_id`. After `Patient/$merge` the source Patient keeps its id with `link = replaced-by`. Deleted records keep their id as tombstones.
3. **One op per change.** Every resource version and every ledger entry has its own `op_id`.
   - Retries reuse the `op_id`, which is the `Idempotency-Key` end to end.
   - Receivers dedup on `op_id` permanently, not for a limited time.
4. **Ids carry no meaning.** A resource id is never a business identifier and is never shown in the UI. A UUIDv7's embedded timestamp is not relied on for ordering. Ordering uses the sync sequence and HLC.

## Deterministic ids for derived records

Anything produced deterministically from another record gets `UUIDv5(platform_namespace, name)`. Replaying the derivation on any node then converges on one id.

| Derivation | Name |
|---|---|
| QuestionnaireResponse extraction | `{QuestionnaireResponse.id}\|{mapping canonical URL, no version}\|{target linkId path}#{repeat index}\|{target resource type}` |
| Any other derivation (charge from a dispense, stock movement from a MedicationDispense, Task from a workflow step, …) | `{source record id or op_id}\|{derivation rule canonical URL}` |

Re-extracting with a newer mapping version keeps the same name, so the result is a new **version** of the same resource, never a second resource. The extraction internals are decided in [How are QuestionnaireResponses extracted into clinical resources?](https://github.com/carapai/people-centered-healthcare/issues/25).

## Business identifiers (Uganda reference)

Business identifiers live in `identifier`. Each system is declared by a NamingSystem in the country configuration package.

| Identifier | Minted by | Offline | Notes |
|---|---|---|---|
| NIN | NIRA (external) | Recorded as entered, status *unverified* | Verification status (`unverified` / `nira-verified` with date) is set only centrally through NIRA. NIN is optional (about 75% coverage). |
| Platform health ID (ECID) | Central, only when a golden record is created | Never minted offline | 12 characters of Crockford base32, random, with a check character, shown in groups of 4. If MoH later mandates a national health ID, it becomes an additional system and the ECID stays. |
| Provisional registration id | The node that registers the Patient | Yes | System `urn:…/provisional/{node_id}`. The Patient also carries `meta.tag = provisional` until central reconciles it. |
| Facility MRN | The Facility Authority Node, or a desktop using its own short code | Yes | Node-prefixed sequence (below). |
| Program numbers (ART, TB, ANC, …) | Per the program's NamingSystem | Yes | A node-prefixed sequence by default. A national sequence uses reserved blocks only if mandated. |
| Receipt, invoice, lab accession and other operational numbers | The minting node | Yes | Node-prefixed sequence. |

## Human-readable numbers (ADR 0014)

- **Default format:** `{facility code}-{node short code}-{sequence}`. The counter lives in the minting node's store and is incremented in the same transaction as the record. Collisions are impossible and the sequence cannot run out.
- **Per-type format:** each identifier type's format is a Published Definition in the country package: prefix, zero-padding, an optional check digit (e.g. Luhn mod N), and an optional yearly reset with a year segment.
- **Who mints:**
  - The Facility Authority Node mints for its facility.
  - A desktop mints under its own short code only while it is the Authority Node for the scope, i.e. as a standalone desktop or under an Authority Lease.
  - Desktop Candidate Versions under an edge receive their number from the edge when accepted, unless the number must be printed offline. In that case the desktop's short code is used.
- **Reserved blocks**, only where an external authority mandates a single central sequence:
  - Central issues blocks to the node, and a low-water alert fires at 20% remaining.
  - On exhaustion the record is still created, with the identifier status `pending`, and central assigns the number on sync.
  - Unused blocks are returned when a node is decommissioned.

## Open items

- The governance of a national health ID (MoH format and issuer) sits in the map's Not yet specified, under the national client registry stance.
- Reserved blocks are sized for the 90-day supported edge offline window ([synchronization.md](synchronization.md)).
