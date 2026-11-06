# health-care-journeys

Layer: **domain** (care-journeys, Patient Engagement). The Care Journey (care-journeys.md; MASTER_PROMPT §61; #115): a longitudinal, patient-centred projection of a person's care.

It is a **read model**. It owns nothing, and each step names its authoritative resource, version and facility. It is rebuilt from those resources whenever it is read.

- **`journey(programs, steps, work) -> Journey`:**
  - one `Stretch` per `Program` (EpisodeOfCare), the earliest start first;
  - then care outside any program, or in one the reader may not see;
  - steps within a stretch the earliest first, then by reference.
- **Steps** (`Stage`): visits, appointments, orders, referrals, results and prescriptions.
- **What waits:** a step is `waiting` while `Work` on it (a Task) is open. Workflow state comes from Tasks, never from clinical status (ADR 0042). `Journey::waiting()` lists them, the earliest first.
- **Rebuildable:** it is a pure function. The same resources in any order, or given twice, make the same journey. A resource given in two versions counts once, at its newer version.

Tests: arranging by program, waiting from workflow not clinical status, order independence and duplicates, and an empty journey.
