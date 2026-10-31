# health-identity

Layer: **domain** (Identity & Access). Pure rules from offline-auth.md § Credentials and § Tokens and sessions. The protocol, hashing and storage are in `health-auth`.

- **`LoginAccount`**: the id, username, an opaque Argon2id PHC string, the status, and a `Lockout`.
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
