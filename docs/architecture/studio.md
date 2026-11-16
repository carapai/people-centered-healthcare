# Studio

This page covers the authoring tools of MASTER_PROMPT §43, §44, §52 and §98: the **Clinical Metadata Studio**, the **Program Designer** and the **Workflow Studio**. What they author is decided elsewhere: packages and their lifecycle in [configuration.md](configuration.md) (ADR 0024, ADR 0025), the artefact types in [metadata.md](metadata.md) (ADR 0049), programs in [program-designer.md](program-designer.md), and workflows in [workflow-engine.md](workflow-engine.md). This page records how people author them. The scope below was decided in [What are the Studio's scope, authors, surface and build order?](https://github.com/carapai/people-centered-healthcare/issues/476). The screens themselves are still being designed on the map.

## Scope

| Question | Decision |
|---|---|
| Who authors | Central experts only: the MoH standards team, the terminology lead, program clinical leads and the platform admin |
| Facility adjustments | Not in the Studio. Facility admins get a separate, small screen that only fills in the override points a package declares (ADR 0026) |
| Where it runs | Screens of the one React app, reachable only when signed in at central with an author permission. Never on an edge server or a desktop. Drafts never sync (ADR 0025) |
| Build order | 1. A shared **package workspace** and the **form designer**. 2. The **Program Designer**. 3. The **Workflow Studio** |
| Workflow editing | A visual canvas of states and transitions (§52), with each element's details edited in a side panel of ordinary form fields. The canvas is `@xyflow/react` 13.x with `@dagrejs/dagre` layout, decided in [Which graph-canvas library fits the Workflow Studio?](https://github.com/carapai/people-centered-healthcare/issues/480) |
| Who may do what | Named permissions granted through roles: `metadata.author`, `metadata.review`, `metadata.approve`, `metadata.publish` and `configuration.activate`. An authoring HTTP API records the signed-in account as the actor, replacing the CLI's `--by <name>` |
| Importing content | Plain FHIR JSON resources and the R4 conversion (metadata.md). FSH/SUSHI sources and an IG Publisher round-trip are not supported |
| Translation of authored content | In scope. English is required; other languages are optional, through FHIR's translation extension. The details are an open ticket |
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

## Previews and simulation

- **Forms** are previewed in the client's own `QuestionnaireRenderer`, the one health workers use, so a preview behaves exactly as the form will.
- **Workflows** are simulated by the Rust engine's sandbox with stubbed actions (ADR 0042). XState stays local to the UI and never decides a transition.

## Open

These are tracked on the map:

- [Studio authoring API](https://github.com/carapai/people-centered-healthcare/issues/477)
- [Form designer](https://github.com/carapai/people-centered-healthcare/issues/479)
- [Workflow Studio](https://github.com/carapai/people-centered-healthcare/issues/481)
- [Program Designer](https://github.com/carapai/people-centered-healthcare/issues/482)
- [Translation](https://github.com/carapai/people-centered-healthcare/issues/483)
- [Facility overrides](https://github.com/carapai/people-centered-healthcare/issues/484)

Still in the fog: terminology browsing and CodeSystem/ValueSet authoring, the decision-table rule builder, the Studio's own profile snapshot generator (ADR 0053), and indicator authoring.
