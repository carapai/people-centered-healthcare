# health-patient-flow-app

Layer: **application** (patient-flow). Queues at the facility authority (#102).

- **`Flow::follow(event)`:** consumes an encounters event after its commit, one transaction each. Entry ids are derived from the event, so a replay changes nothing.
  - `Started`: the patient joins the service point's queue, and an Encounter of class emergency is seen first;
  - `Moved`: they join the queue where they are sent, and their entry where they were is finished;
  - an ended Encounter closes its open entries: done if the patient was called or being seen, else left.
- **`queue(service point)`:** emergencies first, then by arrival.
- **`step(entry, step)`:** call, serve, recall, finish and leave.
- **The queue never changes the Encounter's status.**
