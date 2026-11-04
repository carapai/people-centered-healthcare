# Worklists

Tasks and worklists (workflow-engine.md § Worklists and queues; ADRs 0002, 0044; #99).

- **`WorklistsPage`** (`/worklists`):
  - **Start a workflow:** one of the definitions the node runs, about a context such as `Encounter/123`.
  - **A worklist:** chosen from those the definitions name. Its open tasks are listed most urgent first, then by arrival, and whether each is waiting, yours, or taken by a colleague.
  - **A task:** **Take this task** claims it, and the claim lapses after 15 idle minutes. **Put it back** releases it. **Done: {event}** completes its step with one of the events the step takes.
- **On the server:** `/api/workflow` (`health-fhir-server`'s `workflow_router`) checks `workflow.work` and `workflow.supervise` and runs every command as a workflow command with its `opId`. The screen enforces nothing.
- **Central runs development definitions only** (`HEALTH_DEV_WORKFLOW_DEFINITIONS`) until activation (#89) brings published ones.
- **Not yet:**
  - reassigning and escalating on screen (the API serves them);
  - opening a task's form;
  - live updates (the worklist refreshes after each action).

## Tests

- `test/worklists.test.ts`: the API.
- `e2e/worklists.spec.ts`: a nurse starts a visit, takes the triage task, completes it, and the visit moves to the consultation worklist, with axe on the screen.
