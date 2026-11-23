# Facility configuration

This page consolidates how a facility is configured to operate (MASTER_PROMPT §63–65): its deployment mode, services, funding models, finance steps, stores and local overrides. Every item is decided in the linked document; configuration packaging and lifecycle are in [configuration.md](configuration.md).

## What defines a facility

| Item | Where it lives | Owner | Source |
|---|---|---|---|
| Facility, organization, hierarchy | Facility registry (Organization, Location) | facility-registry | resource-ownership.md |
| Deployment mode and Facility Authority Node (central, edge or standalone desktop) | Facility registry entry | facility-registry | write-authority.md |
| Internal locations: wards, rooms, beds, stores, service points | Facility-scope Locations | facility-registry (FA authority) | write-authority.md |
| Healthcare Services offered | HealthcareService, independent of price | service-catalogue | resource-ownership.md |
| Programs active (ART, TB, ANC, …) | Program activations and override points | configuration | configuration.md |
| Staff and roles | PractitionerRole; locally grantable roles list | workforce-registry, identity | offline-auth.md |
| Devices | Device registry on the edge | edge admin | lan-security.md |
| HMIS org unit ID and the forms reported through the platform | Facility settings, set at central only, checked against the NHFR | reporting | ADR 0069 |

### Places and areas (ADR 0094)

- **In code (#869):** the domain crate `health-facility-registry` holds the hierarchy (`Places`: `area`, a place and everything below it; `facilities_in`, the facilities a district's or region's figures roll up over; `level_of`, with the reserved code `central` the national place), the regions and districts of the national core package (`AdministrativeAreas`, a `Library` of type `administrative-areas`), and the NHFR import (`parse_nhfr_csv` reads an export with `nhfr_id`, `name`, `district` and `status` columns, and optionally `level`, `ownership`, `latitude` and `longitude`; `plan_import` compares it with the registry). `health-registries-app::places` stores each place as an Organization whose id is derived from its code (UUIDv5), with its code under `urn:health:place` (a facility's NHFR identifier also under `urn:health:nhfr`), its level, level of care and ownership as type codings, and `partOf` to the place above; a facility's coordinates are on a Location. Central serves `GET /api/places/area` (the session's place and its area) and `POST /api/places/nhfr/preview` and `/apply` (`places.manage`): the preview lists the regions and districts to add, and the facilities new, changed, closed, unchanged and absent; a district the national core package does not hold refuses the file; applying writes it in one audited transaction. A closed facility becomes `active: false` and stays in its district's area; a facility missing from the file is listed, not closed.
- A PractitionerRole held at a place is a permission snapshot for its code (`central-server provision <user> district-kampala <role>`). Signing in at central for it puts the code in the token's `facility` claim and the level in `place-level` (a facility for a code the registry does not hold, so a facility session is unchanged). The policy decision point refuses an area session every read and search of a patient-level type (the R5 Patient compartment and the facility-scope types), whatever its roles map to.
- **In code (#873):** central's `/api/assignments` assigns roles at a place: `GET /options` (the policy's roles and the administrator's area), `GET /users/{username}`, and `PUT` / `DELETE /users/{username}/{place}`, which sign a new permission snapshot for the account at that place (or remove it), audited in the same transaction. It needs `access.administer` (central-only, never granted offline), and the place must be in the administrator's own area. The client's *Roles at places* and *Import facilities from the NHFR* pages (`features/places`) use it and the NHFR endpoints. District and region boundaries ship in `ug.moh.core` (`Library/administrative-boundaries`, #604; analytics.md § Dashboards).

## Operating models (§63)

One facility may combine several models, for example a free public ward plus a paying private wing. Each model is expressed through configuration, never code:

| Model | Expressed by |
|---|---|
| Government / free | Reference-tariff pricing allocated to a government Funding Source; patient share 0 |
| Private / self-pay | Facility or department Price Books |
| Insurance, with or without copayment | Coverage plus Benefit Plans, and payer-contract Price Books |
| Donor funding, government programs | Funding Sources and `fund` dimensions; donor-eligible stock |
| Employer funding | A payer contract for the employer |
| Subsidies, waivers | Price Rules at stage 5, plus waiver permissions |
| Credit | Receivables with terms |
| Mixed | Coverage order (primary, secondary) and the stage-4 split |

Sources: [pricing-and-insurance.md](pricing-and-insurance.md), [billing-and-payments.md](billing-and-payments.md).

## Configuration scopes and precedence (§64)

| Scope | Examples | How precedence works |
|---|---|---|
| Tenant (country) | Country configuration package: roles, snapshot lifetime, code systems | Base layer |
| Organization, district | District override packages | Override points, key by key (ADR 0026) |
| Facility | Facility override package: services, programs, layouts, queues, gates on or off, set on the Facility settings page (ADR 0067) | Most specific configuration wins |
| Department, service, program | Price Books, gates and charge triggers per department, service or program | Pricing: fixed stages and the most specific Price Book (ADR 0031) |
| Payer contract | Contract Price Books, Benefit Plans | Stage 1 base price and stage 4 split |
| Patient eligibility | Eligibility categories, e.g. an exemption | Stage 5 |
| Transaction | A manual override with permission and reason | Stage 6, audited |

Every change is audited through the lifecycle records (configuration.md).

## Finance steps

- **Financial Gates** are configuration records attached to workflow steps. Override points switch them on or off per facility, and urgent care always defers (ADR 0033).
- **Charge triggers** map clinical events to ChargeItems (billing-and-payments.md).
- **Tills, cashier shifts, merchant accounts** and the facility's chart-of-accounts template are part of facility configuration (ADR 0030, ADR 0032).

## Stores and stock policy

Each store is a facility Location with:
- its stock policy: negative-stock handling, emergency dispense, unleased offline dispensing (inventory-and-procurement.md);
- its valuation method;
- its reorder min/max;
- funding-source eligibility rules.

## What a facility may not change

Facilities cannot change profiles, terminology, Extraction Mappings, Workflow Definitions or clinical rules (ADR 0026). They cannot change prices offline either (ADR 0031).
