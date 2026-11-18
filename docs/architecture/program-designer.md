# Program designer

This page consolidates how clinical programs (ART, TB, ANC, diabetes, hypertension, immunization, …; MASTER_PROMPT §44) are defined and delivered. The definition model is decided, and so is the Program Designer's screen (below). The Studio's scope, workspace and authoring API are in [studio.md](studio.md).

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
- **Imports.** WHO SMART L3 content (R4) is imported and converted to R5. CQL logic is never executed: it is translated into FHIRPath decision tables (guards, and Decision Support Rules), and anything untranslatable is flagged for a person to write (ADR 0043, ADR 0074).
- **Concurrency.** Multiple programs run as separate concurrent Workflow Instances for one patient (ADR 0042).
- **Program context** on records uses `workflow-episodeOfCare` (questionnaire-extraction.md).
- **Transfers.** A program transfer between facilities is a central-coordinated hand-over: the old facility closes its episode and the new one opens its own (write-authority.md).

## Validation before publication

- extraction fixtures and workflow scenarios pass;
- workflow static checks pass (no unreachable or stuck states);
- Owner opt-in for extraction targets;
- semver diff rules;
- the R4 → R5 conversion report has no blocking items.

## The Program Designer

The Program Designer opens from a changed program package on the draft page, like the Studio's other editors. Its layout was decided in [How does the Program Designer assemble a longitudinal program?](https://github.com/carapai/people-centered-healthcare/issues/482), chosen from three prototypes (branch `prototype/program-designer`): the journey board, with the package panel of another variant beside it.

- **It assembles; it does not edit.** Each part of a program is an artefact authored in its own editor: forms in the form designer, the enrolment and care workflows in the Workflow Studio, and regimen and order ActivityDefinitions and Measures in theirs. The Program Designer links them, shows where each one stands, and opens the right editor.
- **Every linked artefact** shows its kind, its `url|version`, and whether it is in this draft, published (pinned to that version), or not created yet. **Open** goes to its editor. **Create** adds a missing one to the draft and opens it.
- **A strip across the top** holds what applies to the whole program: who it is for (the eligibility FHIRPath), the care workflow it runs on, the scopes it is activated for, and how many override points a facility admin may fill in (ADR 0026).
- **The journey board** has one column per stage. The stages are the states of the program's EpisodeOfCare-scoped Workflow Definition, so adding or reordering stages happens in the Workflow Studio. Each column shows:
  - the stage's form;
  - what happens there;
  - its investigations;
  - on the initiation stage, the regimen templates;
  - on follow-up, the schedule;
  - on the outcome stage, the outcomes and indicators;
  - when a patient leaves the stage.
- **Regimen templates** are an ordered list of rules. The first rule that matches the patient chooses the regimen. Each rule shows its FHIRPath condition, the ActivityDefinition it uses, and the approved protocol it cites. Authors reorder rules with up and down buttons.
- **Walk a patient through.** The author enters a sample patient (age, weight, HIV status, TB treatment, pregnancy), and the server's FHIRPath engine evaluates the draft. It shows:
  - whether the patient is eligible;
  - which regimen rule matches;
  - the first year's visit dates, with the investigations due at each.

  This catches rule-order mistakes. In the prototype, a patient on rifampicin was given plain TLD because the general rule came before the TB rule.
- **The package panel** sits beside the board. It shows the package's files by folder (`questionnaires/`, `workflows/`, `activitydefinitions/`, `mappings/`, `measures/`, `tests/`), with the missing ones marked, and a **Still missing** list. *Check now* on the draft page remains the authority.
- **Not designed yet** (not in the prototype):
  - saving a walk-through as a test, and what it would assert;
  - editing the follow-up schedule and investigation timing, which may belong in the care workflow's timers rather than here;
  - authoring the override points a package declares (with the facility-overrides ticket);
  - indicators beyond linking a Measure (the analytics fog);
  - importing a WHO SMART guideline as a program.

## Open (fog)

- The indicator engine, cohorts, and DHIS2 aggregate and tracker export.
