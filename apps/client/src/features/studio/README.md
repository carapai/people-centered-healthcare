# Studio

The Studio's package workspace (studio.md § The package workspace; #492), at `/studio`, only at central. An edge or a desktop shows that the Studio is not there (`capabilities.studio`), and neither serves `/api/studio`.

- **`PackagesPage`** (`/studio`): a DataTable with one row per package, its newest version: title, name and kind; version and semver bump; status; author; **your next step**, or "Nothing for you"; and the scopes it is active in. Enter on a row opens the package.
- **`PackagePage`** (`/studio/$id`, `id` is `name@version`), read from top to bottom:
  1. **What changed:** each added, changed or removed artefact against the previous published version.
  2. **Is it safe?** The stored validation and simulation results, the heading counting the failing ones, each marked when the draft changed since it ran.
  3. **Who must act:** the reviewer, the approvals given and needed, then every action. One the person may not take stays on the page, disabled, with the server's reason, e.g. "You wrote this draft, so you cannot review or approve it". *Request changes* asks why and *Activate* asks for a scope.
  4. **Discussion:** the history. Comments come later.
- **On the server:** `/api/studio` (`central-server`'s `studio` module, over `health-publication::studio`) lists the actions and their reasons from the lifecycle's own rules, and checks the permission, the stage and the separation of duties again on every step (ADR 0025). The client holds no lifecycle rule: it renders the server's list, in the server's order, and words its refusal codes (`wording.ts`).
- **Not yet:** editing, starting a draft, the Draft Holder and handover, *Check now*, comments and the element-level diff.
