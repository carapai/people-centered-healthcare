# health-patient-flow

Layer: **domain** (patient-flow). Queue entries at service points (clinical-workspaces.md; resource-ownership.md; #102). The queue never changes the Encounter's status.

- **`arrive`:** a patient joins a service point's queue when their Encounter starts or moves there. The id, `entry_id`, is derived from the Encounter, the service point and the arrival time, so a replayed event never queues anyone twice.
- **`queue`:** a service point's open entries, emergencies first, then urgent, then routine, each by arrival.
- **`QueueEntry::step`:**
  - waiting → called (by whom) → in service → done;
  - a called patient who does not come is waiting again (recall);
  - a waiting or called patient may leave without being seen.
- **`QueueStore`:** the driven port. `add_entry` is false for an entry already kept.
