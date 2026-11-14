# Equipment is a Supply Chain module, and FHIR `Device` means medical devices only

resource-ownership.md left FHIR `Device` for medical equipment without an Owner. The map's fog listed three candidates: asset management, product-registry and supply chain. The sync protocol had also already taken the type `"Device"` for the edge's enrolled client devices (ADR 0009), sending it in the revocations lane with "revocation wins". The maintainer settled both while resolving [Which module owns FHIR Device instances for medical equipment?](https://github.com/carapai/people-centered-healthcare/issues/452) (2026-11-14).

**The decision:**
- **Equipment** is a physical, individually identified item a facility holds and uses again and again without using it up: a BP machine, an analyser, a fridge. Anything used up stays **Stock**, even if it is a medical device in law (test kits, syringes). See GLOSSARY.md.
- **Owner.** Equipment is owned by a new **equipment** module in Supply Chain, next to inventory and procurement. The module also holds each item's location, operational status and maintenance history.
- **Write authority.** Equipment is facility-scope. The facility's edge is the only writer. Records sync up to central, which reads them for reporting. Moving an item to another facility is an explicit transfer that hands over authority: the sending edge drains its outbox, and central reassigns the facility.
- **Receiving.** A Product is either stocked or tracked as Equipment, never both. Receiving a Product tracked as Equipment creates one Equipment record per unit (serial number or asset tag), in the same transaction as the goods receipt (rule 9). It posts no stock-ledger balance.
- **Loans.** Equipment lent to a patient stays Equipment. The loan is a `DeviceAssociation` owned by clinical-data, and it ends when the item comes back.
- **Patient-assigned Devices.** Devices that become part of a patient's care and do not come back (implants, IUDs) are created by clinical-data, usually from a consumable that was Stock until the procedure.
- **One type, two categories.** FHIR `Device` is split by a platform category in its profile (rule 2): *equipment* goes to equipment, *patient-assigned* to clinical-data. `DeviceDefinition` stays with product-registry.
- **Enrolled client devices leave FHIR `Device`.** The edge's enrolled LAN client devices become a platform sync record, `EnrolledDevice`. It keeps the revocations lane and "revocation wins". FHIR `Device` then means medical devices only, so the resource type alone still picks a version's lane and conflict class.
- **Settled now, built later.** The model is fixed now so the ownership matrix has no gap. Screens, maintenance tracking and valuation for facilities with inventory accounting come with the build issue.

## Considered Options

| Option | Decision |
|---|---|
| **A new `equipment` module in Supply Chain** | **Chosen** |
| inventory | Rejected: its rules are about a quantity ledger (balances, reservations, FEFO), while Equipment is one record per item with its own lifecycle |
| facility-registry | Rejected: Registries are central-authoritative reference data that edges read, and Equipment changes daily at the facility |
| A new Asset Management bounded context | Rejected: too heavy for one module |
| **Rename enrolled client devices to `EnrolledDevice`** | **Chosen** |
| Keep `Device` for all three, with lane and conflict policy branching on category | Rejected: the lane would depend on what is inside the resource, not its type, and a broken BP machine's conflicts would risk being settled by "revocation wins" |

## Consequences

- **Code changes in the build issue:** `health-sync-protocol` `Lane::of` and its test; `health-surrealdb::sync`'s `UP_GLOBAL`, `DOWN_GLOBAL` and conflict-class match; and the ownership registry, which gains `Device` partitioned by category. Nothing is stored as `Device` today, so no data migrates.
- **The product registry** gains a "tracked as Equipment" flag on Product.
- **Inventory's receiving** calls equipment for those Products and skips the stock ledger.
