# Break-glass

The facility in-charge's alerts and the privacy officer's review queue (offline-auth.md § Break-glass; #218).

- **`AlertsPage`** (`/break-glass/alerts`, for `break-glass.alerts`) lists the facility's grants until each is acknowledged.
- **`ReviewsPage`** (`/break-glass/reviews`, for `break-glass.review`) lists the grants awaiting review, the earliest due first, with overdue ones flagged.
  - Selecting a row (Enter, or a click) opens the decision.
  - "Not appropriate" needs a note.
- **Refreshing:** both lists are keyed by the worklists that `/api/events` names (`break-glass-alerts`, `break-glass-reviews`). A new grant, or another reviewer's decision, refreshes an open list without reloading.
- **Refusals:** a 403 is shown as "your role does not include this list", not as an error.
- **`api.ts`** holds the queries and the two posts. `test/break-glass.test.ts` covers them against a fake transport.
