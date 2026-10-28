# Cross-module transactions only inside one Deployable

One application-layer transaction may span several owning modules when they run in the same Deployable (for example, a form submission that writes a QuestionnaireResponse, Observations and a Condition atomically), but each module writes only its own records through its own repository port. Across Deployables, contexts integrate only through events and sagas. We accepted this so that clinically related writes from a single user action stay atomic on the Facility Edge, rather than becoming eventually consistent.

## Consequences

- Extracting a context into its own Deployable turns any such transaction into a saga; use cases that span contexts must be written so that this is possible.
- SurrealDB offers snapshot isolation only, so cross-module invariants still need explicit guards and conflict retry.
