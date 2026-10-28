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
| Facility | Facility override package: services, programs, layouts, queues, gates on or off | Most specific configuration wins |
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
