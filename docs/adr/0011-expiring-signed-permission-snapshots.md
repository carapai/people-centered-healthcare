# Offline permissions are expiring, central-signed snapshots, and offline grants are narrow and provisional

Central grants each user's offline permissions as a signed snapshot per user per facility, carrying a hard `not_after`. The default lifetime is 14 days, configurable from 7 to 30 per country. Only district or central admins extend snapshots, apart from a single audited 7-day emergency extension by the facility in-charge. The edge keeps a monotonic "last known central time" floor and refuses logins when its clock reads earlier than that floor, so moving the clock back cannot defeat expiry. Offline, a facility admin may revoke anything. They may grant only roles on a country-configured "locally grantable" list. These grants are signed by the edge node key and last until central ratifies them, at most 7 days. Central-only roles (administration, privacy officer, sensitive-program access) are never granted offline. This trades two risks against each other: a revoked user can keep working offline until expiry, while staff must not be locked out of patient care during a long outage. The specification forbids indefinitely valid cached permissions.

## Consequences

- When a facility stays offline longer than the snapshot lifetime plus any extension, its staff lose access. Expiry warnings start 3 days ahead.
- On reconnect, the edge pulls revocations and snapshots before uploading audit events and the outbox. Actions by a user who was revoked before the action time are accepted but flagged for review, never discarded.
