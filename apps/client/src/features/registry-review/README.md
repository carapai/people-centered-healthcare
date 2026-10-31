# Registry review

The registry review queue (patient-identity.md § Review queue; #203), at `/registry/reviews`, for reviewers whose roles map to `registry.review`.

- **The queue** is a DataTable of pending items, the oldest first, with overdue items (over 7 days) flagged. It is moved through with the arrow keys, and Enter opens an item.
- **A possible duplicate** shows the new registration and the existing record side by side, field by field, marked as agreeing or differing, with the match weight. The actions are **Same person** (link) and **Different people** (never linked automatically again).
- **A disagreeing value** shows the current and incoming values with their trust and source record. The actions are **Keep current** and **Take incoming**.
- **On the server:** every decision is checked, carried out and audited at `/api/registry/reviews` (`health-fhir-server`'s `registry_review_router`). The screen enforces nothing on its own.
- **Not yet:**
  - unlinking from a golden record's own view, which comes with the patient record screens (`Patient/{id}/$unlink` is served);
  - an end-to-end decision scenario. `e2e/registration.spec.ts` checks the screen with axe (#228).
