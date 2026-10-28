# Program designer

This page consolidates how clinical programs (ART, TB, ANC, diabetes, hypertension, immunization, …; MASTER_PROMPT §44) are defined and delivered. The definition model is decided; the **Program Designer UX is not**, and stays in the map's Studio fog.

## A program is a package

A program is a **program package** (ADR 0024) authored and published through the §45 lifecycle (configuration.md). It contains:

| §44 element | Represented as | Source |
|---|---|---|
| Program identity | EpisodeOfCare type (a local ValueSet code) | research/clinical-workflows.md |
| Eligibility, enrollment | FHIRPath eligibility rules plus an enrollment Workflow Definition | workflow-engine.md |
| Stages, clinical activities, follow-ups, review, referral, outcomes | An EpisodeOfCare-scoped Workflow Definition (states, timers, lost-to-follow-up escalation, invoked sub-workflows) | workflow-engine.md |
| Forms | Questionnaires plus Form Layouts | metadata.md, configuration.md |
| Data capture → clinical records | Extraction Mappings | questionnaire-extraction.md |
| Investigations, orders | ActivityDefinitions used by typed create-request actions | workflow-engine.md |
| Regimen templates | ActivityDefinitions or MedicationRequest templates that reference approved protocol sources. Regimens are never hardcoded (§44) | metadata.md |
| Workflow rules | FHIRPath guards and decision tables (WHO SMART DAK) | ADR 0043 |
| Reporting indicators | Measures; the indicator engine is in the analytics fog | metadata.md |
| Facility applicability | Program activation per scope, plus override points | ADR 0025, ADR 0026 |
| Financing | Program Price Books, Funding Sources, donor-eligible stock | facility-configuration.md |

## Rules

- **Approved sources.** Program packages reference approved clinical protocols and versioned metadata, and need two approvers for clinical content (ADR 0025).
- **Imports.** WHO SMART L3 content (R4) is imported and converted to R5. CQL logic is not executed in workflow guards, so it is translated into FHIRPath decision tables where possible; CQL for decision support remains in the fog (ADR 0043).
- **Concurrency.** Multiple programs run as separate concurrent Workflow Instances for one patient (ADR 0042).
- **Program context** on records uses `workflow-episodeOfCare` (questionnaire-extraction.md).
- **Transfers.** A program transfer between facilities is a central-coordinated hand-over: the old facility closes its episode and the new one opens its own (write-authority.md).

## Validation before publication

- extraction fixtures and workflow scenarios pass;
- workflow static checks pass (no unreachable or stuck states);
- Owner opt-in for extraction targets;
- semver diff rules;
- the R4 → R5 conversion report has no blocking items.

## Open (fog)

- Program Designer and Workflow Studio UX (prototype tickets expected).
- The indicator engine, cohorts, and DHIS2 aggregate and tracker export.
- CQL for decision support.
