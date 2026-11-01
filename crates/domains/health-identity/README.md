# health-identity

Layer: **domain** (Identity & Access). Pure rules from offline-auth.md § Credentials and § Tokens and sessions. The protocol, hashing and storage are in `health-auth`.

- **`LoginAccount`**: the id, username, an opaque Argon2id PHC string, the status, and a `Lockout`.
- **`PhoneAccount`** (#111; ADR 0029): a patient's Login Account, one per verified phone number. It has no password: each sign-in is a one-time code to the phone. It reaches patient data only through a Person (`health-client-registry`), never because a Patient's phone number matches.
  - **`PhoneNumber`:** E.164. `parse` accepts what people write (`+256 772 123 456`, `00256…`, or the national `0772…` with the country code given) and refuses letters and lengths outside 8–15 digits. Stored numbers deserialize only in E.164.
  - **`OtpPolicy` / `OtpChallenge`:** six digits, valid 5 minutes, 5 tries. One code a minute and 5 an hour per phone (`may_send`). `attempt` counts every try, refuses an expired, used or spent challenge, and accepts a code once. The code is kept only as a hash that this crate never reads.
- **`LockoutPolicy` / `Lockout`**:
  - every 10th consecutive failure locks the account for 15 minutes;
  - 100 consecutive failures lock it until an administrator unlocks it;
  - a success resets the count.
- **`PasswordPolicy`**:
  - a new password needs at least 8 characters on an enrolled device and 15 without one, counted in characters rather than bytes;
  - a password on the breached-password list is refused (the list is [#189](https://github.com/carapai/people-centered-healthcare/issues/189)).
- **`SessionPolicy`**:
  - access tokens last 15 minutes;
  - a session is refreshable until 12 hours after its password login;
  - no access token outlives that cap.
- **`Scopes`**: SMART App Launch v2 scopes.
  - Resource scopes are `{patient|user|system}/{Type|*}.{cruds}[?query]`, with the letters in order and none repeated.
  - SMART v1 forms are refused.
  - `granted()` drops scopes this server does not grant yet, such as launch context.
  - `permits()` answers coarse grants without a query restriction. Query restrictions are the policy decision point's.
- **`snapshot`**: permission snapshots (ADR 0011).
  - **`PermissionSnapshot`:** the account, the facility, the roles (each optionally limited to some locations), the program assignments, `issued_at` (central time), `not_after` and `snapshot_version`.
  - **`SnapshotPolicy`:** a 14-day default lifetime, configurable from 7 to 30 days. `check` refuses an expired snapshot or one dated in the future, and reports `ExpiresSoon` for the last 3 days.
  - **`ClockFloor`:** the last known central time. It only moves forward, and `check` refuses a local clock that reads before it.
  - **`RolePermissions`:** role → permission mappings from the configuration package. `validate` refuses unknown roles (central will not sign them), and `resolve` gives the union of a snapshot's permissions.
