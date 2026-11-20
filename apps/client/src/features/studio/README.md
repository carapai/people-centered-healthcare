# Studio

The Studio's package workspace (studio.md § The package workspace; #492), at `/studio`, only at central. An edge or a desktop shows that the Studio is not there (`capabilities.studio`), and neither serves `/api/studio`.

- **`PackagesPage`** (`/studio`): a DataTable with one row per package, its newest version not withdrawn: title, name and kind; version and semver bump; status; its Draft Holder; **your next step**, or "Nothing for you"; and the scopes it is active in. A click on a row, or Enter, opens the package.
- **`PackagePage`** (`/studio/$id`, `id` is `name@version`), read from top to bottom:
  1. **What changed:** each added, changed or removed artefact against the previous published version.
  2. **Is it safe?** The stored validation and simulation results, the heading counting the failing ones, each marked when the draft changed since it ran.
  3. **Who must act:** the reviewer, the approvals given and needed, then every action. One the person may not take stays on the page, disabled, with the server's reason, e.g. "You wrote this draft, so you cannot review or approve it". *Request changes* asks why and *Activate* asks for a scope.
  4. **Discussion:** the history. Comments come later.

  Under the title it names the **Draft Holder** (#493), and a handover waiting to be accepted. Only the holder edits, checks, submits, hands over or withdraws the draft. *Hand over* asks for a username, and the draft stays with its holder until that person presses *Accept the handover* (their next step); either of them may *Call off the handover*. *Reassign*, with `metadata.manage`, gives the draft to a username at once and asks why. *Withdraw* ends the draft: the page moves to the id it is kept under, read only, and its version can be drafted again. Everyone who has held the draft is an author, so none of them may review or approve it.

  It also lists the package's **files**, each opening in the file editor. In a draft the person may add a file and change the version; a published version offers *Start the next version*, a copy at the next minor `-draft` version. *Check now* is one of the actions: it stores validation and simulation results and leaves the draft a draft.
- **`FileEditor`** (`/studio/$id/files/$`, the splat is the file's path): one file as JSON text. *Save* sends that one file with the revision it was read at as `If-Match`; if someone saved the draft since, the server answers 412, nothing is kept, and the person reloads. A path the draft does not hold opens empty and is added on its first save. Read-only when the server says the person may not edit (`editable`, `editRefusal`).
- **`PackagesPage`** also starts a blank new package (name, title, kind, first version, canonical base).
- **On the server:** `/api/studio` (`central-server`'s `studio` module, over `health-publication::studio`) lists the actions and their reasons from the lifecycle's own rules, and checks the permission, the stage and the separation of duties again on every step (ADR 0025). The client holds no lifecycle rule: it renders the server's list, in the server's order, and words its refusal codes (`wording.ts`).
- **Not yet:** comments, the element-level diff, and the structured editors (form designer, Workflow Studio and the others), which will open from a file in place of the JSON editor.
