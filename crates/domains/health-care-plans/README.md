# health-care-plans

Layer: **domain** (care-plans). Care plans (resource-ownership.md: CarePlan, Goal and CareTeam are care-plans'; #110). The basics:

- **`draft`:** a patient's plan with its title and what it is for, starting as a draft.
- **`CarePlan::transition`** follows R5:
  - a draft becomes active, or is revoked;
  - an active plan is held, completed or revoked, and a held one resumes or is revoked;
  - an open one may be entered in error.
- **`CarePlan::address`:** the patient's Conditions the plan addresses.
- **`CarePlan::plan`:** its activities, the patient's own requests (`ACTIVITY_TYPES`: ServiceRequest, MedicationRequest, Task, Appointment, NutritionOrder, DeviceRequest), each once. This is R5's `activity.plannedActivityReference`, which replaced the inline activity detail.
- **A plan that has ended** takes no more.
