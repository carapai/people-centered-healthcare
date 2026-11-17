# Studio

This page covers the authoring tools of MASTER_PROMPT §43, §44, §52 and §98: the **Clinical Metadata Studio**, the **Program Designer** and the **Workflow Studio**. What they author is decided elsewhere: packages and their lifecycle in [configuration.md](configuration.md) (ADR 0024, ADR 0025), the artefact types in [metadata.md](metadata.md) (ADR 0049), programs in [program-designer.md](program-designer.md), and workflows in [workflow-engine.md](workflow-engine.md). This page records how people author them. The scope below was decided in [What are the Studio's scope, authors, surface and build order?](https://github.com/carapai/people-centered-healthcare/issues/476). The Program Designer's screen is in [program-designer.md](program-designer.md#the-program-designer); the remaining screens are still being designed on the map.

## Scope

| Question | Decision |
|---|---|
| Who authors | Central experts only: the MoH standards team, the terminology lead, program clinical leads and the platform admin |
| Facility adjustments | Not in the Studio. Facility admins use the **Facility settings** page at central, which fills in only the override points a package declares (ADR 0026, ADR 0067; configuration.md) |
| Where it runs | Screens of the one React app, reachable only when signed in at central with an author permission. Never on an edge server or a desktop. Drafts never sync (ADR 0025) |
| Build order | 1. A shared **package workspace** and the **form designer**. 2. The **Program Designer**. 3. The **Workflow Studio** |
| Workflow editing | A visual canvas of states and transitions (§52), with each element's details edited in a side panel of ordinary form fields. The canvas is `@xyflow/react` 13.x with `@dagrejs/dagre` layout, decided in [Which graph-canvas library fits the Workflow Studio?](https://github.com/carapai/people-centered-healthcare/issues/480) |
| Who may do what | Named permissions granted through roles (see the authoring API below). The API records the signed-in account as the actor |
| Importing content | Plain FHIR JSON resources and the R4 conversion (metadata.md). FSH/SUSHI sources and an IG Publisher round-trip are not supported. Profiles must arrive with their snapshots, produced outside the product ([profile-snapshots.md](../research/profile-snapshots.md)) |
| Translation of authored content | In scope. English is required; other languages ship as Language Packs (ADR 0066), edited in the translation editor below |
| Working on a draft | One author at a time. Others read and comment, and the author can hand the draft over. There is no live co-editing |

## The package workspace

Every Studio screen sits inside the package workspace. It carries a draft through the §45 lifecycle that health-publication already enforces: validation and simulation, submission, review and changes requested, approval under the separation-of-duties rules (ADR 0025), publication, and activation per scope. Its layout was decided in [How does the Studio's package workspace look and flow?](https://github.com/carapai/people-centered-healthcare/issues/478), chosen from three prototypes (branch `prototype/studio-workspace`).

- **The package list** is a table with one row per package. Each row shows:
  - the title, name and kind;
  - the version and its semver bump;
  - the status and the author;
  - **your next step**: the first action the signed-in person may take, or "Nothing for you";
  - the scopes the package is active in.
- **A draft is one page, read from top to bottom:**
  1. **What changed.** Each added, changed or removed artefact compared with the previous version, with a plain summary (the semver diff of metadata.md). Unchanged artefacts are left out.
  2. **Is it safe?** The validation and simulation results. The heading counts the failing ones.
  3. **Who must act.** The approvers that the package's kind needs, who has approved so far, and then the actions. An action the signed-in person may not take stays on the page, disabled, with the reason: for example "You wrote this draft, so you cannot review or approve it", or "1 more approval needed".
  4. **Discussion.** The history (created, edited, submitted, reviewed, approved, handed over) with its comments, and a box for a new comment.
- **Editors** (the form designer, the Workflow Studio, and the others) open from a changed artefact. They are not embedded in the draft page.

The authoring API therefore gives the following for each package: the actions the signed-in person may take, with the reason for each one refused; the artefact diff against the previous version; the check results; and the history with comments.

## The authoring API

The Studio's HTTP API runs at central only, under `/api/studio/`, and acts as the signed-in account. It was decided in [What is the Studio's authoring API and who may call it?](https://github.com/carapai/people-centered-healthcare/issues/477).

- **Draft Holder.** A draft has one holder at a time (GLOSSARY.md). Only the holder edits, checks, submits, hands over or withdraws it.
  - A handover takes effect only when the new holder accepts it. Until then the current holder keeps the draft.
  - A person with `metadata.manage` can reassign a draft whose holder is unavailable. They must give a reason, which goes into the discussion and the audit.
  - Everyone who has held a draft counts as its author, so none of them may review or approve it (ADR 0025). This replaces the rule that anyone who uploads a revision becomes a co-author.
- **Editing.** The server keeps the draft's working copy of files. Editors save one artefact at a time, and the server repacks the tarball after each save. Each draft has a revision, sent as an ETag. A save must carry `If-Match`, and a stale one is refused with 412, so a person reloads instead of overwriting someone else's change.
- **Starting.** A draft starts blank, for a new package, or as a copy of a published version with the next version proposed. The holder may change the version until submission, where the semver check confirms it.
- **Checks.** *Check now* runs the same validation and simulation as submission, and stores the latest results without changing the stage. Submission runs them again, and that run decides.
- **Allowed actions.** For each package, the server lists the actions the signed-in person may take, and the reason for each one refused. It derives these from the rules that enforce them. The client only displays them.
- **What changed.** The server works out, against the latest published version, which artefacts were added, changed or removed, and which element paths changed in each. The summaries are generated from that. The holder may add one change note for the whole draft.
- **Discussion.** Each draft has one thread. A comment may name the artefact it is about.
- **Withdrawal.** The holder may withdraw an unpublished draft. It is kept, with its history, and its `name@version` is free for a new draft. Nothing is deleted.
- **Permissions**, on top of the separation of duties. There is no restriction by package kind in v1.

  | Permission | Allows |
  |---|---|
  | `metadata.author` | start, edit, check, submit, hand over, withdraw |
  | `metadata.review` | take a review, request changes |
  | `metadata.approve` | approve |
  | `metadata.publish` | publish |
  | `configuration.activate` | activate |
  | `metadata.manage` | reassign a draft |

- **The CLI** (`central-server package …`) stays, for bootstrap and CI. Its `--by` name is audited as a CLI action.

## The form designer

The form designer opens from a changed Questionnaire on the draft page. Its layout was decided in [How does the form designer author a Questionnaire and its Form Layout?](https://github.com/carapai/people-centered-healthcare/issues/479), chosen from three prototypes (branch `prototype/form-designer`).

- **Three columns:**
  - **Left:** the questions, grouped by Form Layout section, with reordering and *Add question*. A question in no section is listed as such and shows at the end of the form, because a layout cannot hide a question.
  - **Middle:** an inspector for the selected question.
  - **Right:** a live preview in the real `QuestionnaireRenderer`, re-rendered on every change.
- **The inspector keeps the three artefacts apart**, in three groups:
  1. **Question**, the clinical part that needs approval: text, id (fixed once published), answer type, required, options or a bound ValueSet, more than one answer, and *shown only when*.
  2. **Layout**, the presentation: section, widget, hint.
  3. **Becomes a record**, the extraction: the target resource and its code.
- **Problems while editing** are listed above the editor: duplicate ids, a choice with no options, a condition on a question that is not yes/no, a widget that does not fit, a question in no section. *Check now* remains the authority.
- **Import FHIR JSON** replaces the form with a pasted Questionnaire. R4 content goes through the conversion report.
- **Not designed yet** (not in the prototype):
  - `enableWhen` operators other than "is Yes", and several conditions;
  - FHIRPath expressions for calculated values and constraints;
  - nested groups and repeating groups.
- **Binding a ValueSet** uses the picker described under Terminology below.

## The Workflow Studio

The Workflow Studio opens from a changed Workflow Definition on the draft page. Its layout was decided in [How does the Workflow Studio edit, simulate and publish a statechart?](https://github.com/carapai/people-centered-healthcare/issues/481), chosen from three prototypes (branch `prototype/workflow-studio`).

- **Three columns:**
  - **Left: the outline.** The states as a tree that shows their nesting, with each state's exits listed under it (event, guard, target). Timers are marked. Selecting a row selects the same element on the canvas.
  - **Middle: the canvas.** The statechart drawn on `@xyflow/react`, laid out left to right with `@dagrejs/dagre`. Compound and parallel states are group nodes, and a parallel state's regions are drawn inside it. Dragging from one state to another adds a transition. Problems are outlined in red.
  - **Right: one panel with two modes,** *Inspect* and *Sandbox*.
- **Inspect** edits the selected element in ordinary form fields:
  - **A state:**
    - its name and its id, which is fixed once published, because running instances and migration maps refer to it;
    - its kind (atomic, compound, parallel, final), the state it sits inside, and, for a compound state, the child it starts in;
    - what happens there: wait for an event, a human task (role, worklist, form `url|version`, priority, claim timeout), a payment gate (a Financial Gate configuration), or a sub-workflow `url|version`;
    - its entry actions;
    - the transitions that leave it.
  - **A transition:**
    - its source and target;
    - what triggers it: an event, a timer (an ISO 8601 duration, run as the workflow principal), or nothing (eventless);
    - its guard: always, a FHIRPath expression (the panel lists the declared data it may read), or a decision table's outcome, with the table's rows edited in place;
    - the other transitions on the same event, in document order, because the first whose guard holds wins;
    - its actions.
  - **Typed actions** are picked from the registered list. Each shows the permission it needs and its compensation, and its parameters are fields generated from its parameter schema (ADR 0043).
- **Sandbox** plays the draft in the Rust engine's simulation at central (`health_workflow::simulation`), with every action stubbed:
  - the author sets each guard's answer, then sends any event the current configuration accepts, or waits for a timer;
  - steps can be undone, and the run restarted;
  - after each step it shows the configuration, the stubbed actions, the open tasks and the pending timers, with the active states lit on the canvas and in the outline, and the history so far;
  - **Save as scenario** writes the run to the package's `tests/` folder as a `workflow-scenario` file: the guard stubs, the steps, and the configuration reached at each step as its expectation.
- **Publication checks** are listed above the editor and rerun as the author edits (workflow-engine.md, "Simulation and publication gates"): the scenarios pass, every state can be entered, every non-final state has a way out, actions and their parameters are valid, guards read only declared data, and the workflow principal's permissions cover its timers and eventless steps. Each problem selects the element it is about. A permission the principal lacks can be declared from there, and it is then approved at publication. As with the form designer, *Check now* on the draft page remains the authority.
- **The canvas keeps React Flow's attribution link.** MIT does not require it, and the maintainers ask that only React Flow Pro subscribers remove it. There is no subscription.
- **For the build:**
  - `@xyflow/react` 13.0.1's own type declarations fail `tsc` under the client's `exactOptionalPropertyTypes` with `skipLibCheck: false` (`InternalNode`'s `sourcePosition`). This needs an upstream fix or a local type patch before it is adopted.
  - v13 names the theme prop `forceColorMode`.
  - Zooming on scroll is off, so the page scrolls over the canvas.
- **Not designed yet** (not in the prototype):
  - editing the data model and the data requirements;
  - adding a decision table;
  - opening a saved scenario in the sandbox, and editing its expected actions, tasks and timers;
  - error exits of a sub-workflow and the cancellation and compensation paths;
  - the migration map from the previous version;
  - the PlanDefinition import's review screen.

## The translation editor

The translation editor opens from a Language Pack draft (ADR 0066). It was decided in [How are forms and code lists translated?](https://github.com/carapai/people-centered-healthcare/issues/483).

- **A table per artefact** of the target package: English on the left, the pack's language on the right, one row per translatable string (item text, prefix, help, answer options, section titles, code-list displays).
- **Filters** for missing and stale strings, and each artefact's coverage. A stale row shows the old English beside the new.
- **A live preview** of each form in the pack's language, in the real `QuestionnaireRenderer`, falling back to English string by string as health workers and patients will see it.
- The pack is submitted, approved and activated like any package, with one approver who reads the language.

## Terminology

The terminology screens open from a changed CodeSystem or ValueSet on the draft page. Their layout was decided in [How does the Studio browse terminology and author CodeSystems and ValueSets?](https://github.com/carapai/people-centered-healthcare/issues/512), chosen from three prototypes (branch `prototype/terminology-studio`).

- **Three columns:**
  - **Left: the code systems** that are active, each marked local or external (read-only, shipped as a terminology package; terminology.md), with its version and concept count.
  - **Middle: the hierarchy** of the selected code system, expandable, with search by code or words. Search runs in the terminology service, because external systems such as ICD-11 hold tens of thousands of concepts. The selected concept shows its display, its definition and the ValueSets that contain it.
  - **Right: the open ValueSet**: its expansion, the change against its published expansion, and the forms that bind it.
- **Adding to a ValueSet.** With a draft ValueSet open, the selected concept can be:
  - added alone;
  - added with everything under it, as an `is-a` filter, so later child codes follow;
  - excluded.

  Concepts already in the ValueSet are ticked in the tree.
- **The change against published** lists codes added and removed. Removed ones are flagged: records that already hold them stay valid against the version that issued them, and new forms no longer offer them.
- **Binding from the form designer.** The form designer's picker searches ValueSets (published, or drafts in the same package), previews the codes and suggests a widget: radio buttons up to 7 codes, a drop-down up to 30, and search-as-you-type beyond that. Binding records `url|version`, and publication pins the expansion in the manifest.
- **Editing a local CodeSystem** happens in the same browser, decided in [How does the Studio edit a local CodeSystem's concepts?](https://github.com/carapai/people-centered-healthcare/issues/519), chosen from three prototypes (branch `prototype/codesystem-editor`, variant A). External code systems stay read-only.
  - The tree gains **Add a code** and **+ under** on each concept. The concept panel becomes a form: code, display, definition and parent.
  - **A code is fixed once published.** A new code can still be renamed or deleted; a published one can only be **retired**: kept, so records that hold it still resolve, but no longer offered on forms.
  - Beside the tree, **Version** shows the next version and why, worked out from the change by terminology.md's rules: adding or retiring a code is minor, a display change a patch, and redefining or removing a published code major (removal is not offered).
  - **What it reaches** lists each ValueSet that draws on the code system, with the codes it gains or loses, and the forms that bind it.
  - An optional note on the change says why (for example *IMCI 2024 adds this sign*); reviewers read it in the draft's discussion.
- **Not designed yet** (not in the prototypes):
  - moving a published code under another parent (it changes what `is-a` ValueSets expand to; its version bump is not set);
  - entering many codes at once;
  - filters other than `is-a`;
  - ValueSets that span versions of a code system;
  - ConceptMap authoring.

## Decision tables

The decision-table editor opens from the Workflow Studio, from a guard that uses a table or from the definition's tables. It was decided in [How does the Studio author and test a decision table?](https://github.com/carapai/people-centered-healthcare/issues/513), chosen from four prototypes (branch `prototype/decision-tables`, variant D).

- **The rules list** shows every rule in order. The first rule that holds wins. Each rule has a **name** and an outcome, with markers for *fires for this patient* and *never fires*, reorder buttons and *Add rule*.
- **Clicking a rule** opens its detail to edit: its name, its outcome, *Delete rule*, and its condition as a **tree**.
  - Groups join their children with **all of these**, **any of these** or **not** (none of these), and nest to any depth.
  - Conditions pick an input from the workflow's declared data requirements, an operator and a value.
  - The tree is saved as **one FHIRPath expression** (`and`, `or`, `.not()`), shown under it. The engine's table keeps its shape: a row's `when` holds that expression, and the row gains an optional `name`.
- **Try a patient.** The server evaluates the draft table for sample inputs. Every node is marked as holding or not, and the outcome links to the rule that fired. *Save as test* keeps the inputs and today's outcome as a test case in the package's `tests/`, and submission's simulation runs it.
- **Checks** above the editor, rerun as the author edits:
  - inputs no rule covers, when the table has no default;
  - rules that never fire because a rule above always matches first;
  - failing test cases.

  A default outcome can be set. *Check now* on the draft page remains the authority.
- **For the build:** the client's lint forbids a property named `then` (`no-thenable`), which is what the engine calls a row's outcome, so the client maps it at its boundary.

## Indicators

The indicator editor opens from a changed `Measure` on the draft page; the Program Designer's package panel also lists the program's indicators beside the journeys they count. Its behaviour was decided in [How does the Studio author an indicator and test it against sample patients?](https://github.com/carapai/people-centered-healthcare/issues/530), its layout in [How does the indicator editor look: populations, period date, disaggregations and Try?](https://github.com/carapai/people-centered-healthcare/issues/534), and its calculations, dates and disaggregations in [How does the indicator editor filter by dates relative to the reporting period, and author age bands and coded disaggregations?](https://github.com/carapai/people-centered-healthcare/issues/539), from prototypes (branch `prototype/indicator-editor`). Indicators are defined and computed as in [analytics.md](analytics.md) (ADR 0068).

- **Layout:** the definition on the left (period date and disaggregations, *What it calculates*, the numerator, then the denominator), and *Try* beside it on the right, always visible.
- **Indicators have no period of their own.** A **Report** groups indicators and chooses the period type (weekly, monthly, quarterly, semi-annual, annual, financial year from July) and the period; every indicator in it is evaluated over that period. How a Report is defined is still open ([#540](https://github.com/carapai/people-centered-healthcare/issues/540)).
- **What it calculates:**

  | Calculation | Needs a denominator | FHIR `Measure` scoring |
  |---|---|---|
  | Count | No | cohort |
  | Percentage (× 100) | Yes | proportion |
  | Rate (× 100, 1,000, 10,000 or 100,000) | Yes | ratio; the factor (e.g. per 1,000) is recorded on the Measure as an extension |
  | Ratio | Yes | ratio |
  | Sum, average, median, minimum, maximum of a value | No | continuous-variable, the value as the measure observation |
  | Count distinct values of a coded field | No | continuous-variable |

  - **What is counted:** people (each once), enrollments (each once), visits, or observations.
  - **A value** is a numeric field (e.g. gestational age) or the days between two dates (e.g. last menstrual period to the visit). Counting people or enrollments, it is each one's **first** or **last** value in the period.
  - **Cumulative** counts from the first record up to the end of the reporting period (e.g. *ever enrolled on ART*), instead of within it.
- **Numerator and denominator.** The denominator appears only for percentage, rate and ratio. It is either the counted things with care in the period that match its conditions, or a **Population Estimate** type for the place, its annual figure taken pro rata for the period (e.g. expected pregnancies; [ADR 0071](../adr/0071-population-estimates-ship-as-yearly-population-packages.md)); an estimate can't be disaggregated. The numerator is the counted things in the denominator that match its own conditions, or everything with care in the period when there is no denominator from records. There is no separate exclusion population: a *none of these* group does that.
- **Criteria** use the decision tables' condition tree (all / any / not), saved as one FHIRPath expression shown beneath it. Raw FHIRPath is the fallback for what the tree can't express. A group can add:
  - **+ condition** over the program's declared data;
  - **+ date** on any declared date (visit, observation, enrollment, episode of care start or end, last menstrual period, date of birth, next appointment, …): *is in the reporting period*, *is on*, *before* or *after* the period's start, its end or a fixed date, *is between* two of those, *is within* N days, weeks or months before or after one, *is in the previous* N periods (of the Report's period type), *is empty*, *is not empty*. They compile against `%periodStart` and `%periodEnd`;
  - **+ group**;
  - **+ in another indicator**: *is in* another published indicator's numerator or denominator, pinned to that version until the author changes it. Indicators that refer to each other in a circle are refused at Validation.
- **The period date** is required: the declared date that places care in a Report's period (e.g. when the visit started), from the same list as date conditions.
- **Disaggregations:**
  - **Age bands** come from an **Age Band Set**, its own published configuration (national core or a program package, [ADR 0071](../adr/0071-population-estimates-ship-as-yearly-population-packages.md)), which fixes the unit and the date age is taken at. It is edited in its own small table (name, unit, age-at date, from–to rows, gaps and overlaps flagged). The indicator only picks one ([#541](https://github.com/carapai/people-centered-healthcare/issues/541)).
  - **Any coded field** (sex, HIV status, visit type, district, …) disaggregates by its codes.
  - Every disaggregation has a **not recorded** band, so the bands always add up to the total.
- **Try** picks a period type and a period as a Report would, and shows the value, the numerator and denominator per disaggregation band, and who is in each side and why. *Save as test* writes the expected value to `tests/`, and submission's checks fail when a later edit changes it.
- **HMIS mapping** is edited in the Reporting package (ADR 0069). The editor only shows, read-only, where the indicator is mapped (e.g. *HMIS 105, ANC 1st visit*).

## Previews and simulation

- **Forms** are previewed in the client's own `QuestionnaireRenderer`, the one health workers use, so a preview behaves exactly as the form will.
- **Workflows** are simulated by the Rust engine's sandbox with stubbed actions (ADR 0042), through the Workflow Studio's sandbox. XState stays local to the UI and never decides a transition.

## Open

The Studio has no profile snapshot generator ([profile-snapshots.md](../research/profile-snapshots.md)).
