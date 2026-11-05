# health-encounters

Layer: **domain** (encounters). Check-in and the Encounter lifecycle (clinical-workspaces.md; resource-ownership.md; #102). The Encounter's status is the real clinical status: only this module changes it, never a workflow or a queue.

- **`check_in`:** starts an `in-progress` Encounter at a service point, with its `EncounterEvent::Started`. `open_guard(patient, service point)` is the guard key that keeps one open Encounter per patient per service point. The application claims it with the Encounter and releases it when the Encounter ends.
- **`Encounter::transition`** follows R5's codes:
  - planned → in-progress or cancelled;
  - in-progress → on-hold, discharged, completed or discontinued;
  - on-hold → in-progress or discontinued;
  - discharged → completed;
  - anything not yet final → entered-in-error.

  Completed, cancelled, discontinued and entered-in-error are final.
- **`Encounter::move_to`:** sends a patient under way to another service point (`EncounterEvent::Moved`).
- **`EncounterEvent`s** go on the outbox (`encounters.started.v1`, `encounters.moved.v1`, `encounters.status-changed.v1`). Patient-flow follows them.
- **`EncounterStore`:** the driven port.
