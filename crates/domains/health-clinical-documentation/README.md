# health-clinical-documentation

Layer: **domain** (clinical-documentation). Clinical notes (clinical-workspaces.md § Signatures and amendments; synchronization.md; #107). Notes are Compositions, and DocumentReferences, that clinical-documentation owns.

- **`NoteKind`:** the kinds of note, each with its LOINC document type and starting sections:
  - progress (SOAP);
  - consultation;
  - admission;
  - procedure;
  - discharge;
  - letter.
- **`Note`:** the lifecycle.
  - A draft (`preliminary`) is edited, or cancelled.
  - Signing makes it `final`, with who signed and when.
  - **`amend`:** a change to a signed note is a new note that replaces it (`amended`) or appends to it (`appended`). It is signed as it is made, and it relates to the original, which is never edited.
- **`may_replace` and `may_delete`:** the rule every write applies.
  - A signed version (`final`, `amended`, `appended`, `corrected`) is never replaced in place, except to mark it `entered-in-error` with its content unchanged.
  - It is never deleted.
  - The writing node and the Authority Node apply the same rule: synchronization.md rejects an in-place edit of a signed version at the receiver.
