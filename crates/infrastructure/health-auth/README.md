# health-auth

Layer: **infrastructure**. The OIDC / SMART App Launch v2 authorization server (offline-auth.md, ADR 0010). Central and every edge run this same code: central accepts bearer or DPoP tokens, and an edge requires DPoP (`AuthConfig::require_dpop`).

## Endpoints

`AuthServer::router()` provides these, relative to the issuer URL:

| Route | What it does |
|---|---|
| `GET /.well-known/openid-configuration`, `/.well-known/smart-configuration` | Server metadata: S256 PKCE only, public clients, ES256 for DPoP and ID tokens, SMART capabilities |
| `GET /jwks` | The ES256 public keys: the active one and any rotated-out keys whose tokens may still be live. Each `kid` is the RFC 7638 thumbprint |
| `GET /authorize` | Validates the request and shows the sign-in form |
| `POST /authorize` | Checks the credentials and redirects with a one-time code, `state` and `iss` (RFC 9207) |
| `POST /token` | `authorization_code` (PKCE S256) and `refresh_token` grants |
| `POST /revoke` | Logout (RFC 7009): either token ends the session |

- **Requests:**
  - An unknown client or redirect URI gets an error page and is never redirected to. Other request errors are redirected back.
  - PKCE is required, and `plain` is refused. There is no password grant.
- **Sign-in:**
  - Every refusal shows one message, and an unknown username still spends one Argon2id verification, so responses reveal neither whether an account exists nor whether it is locked.
  - Lockout follows `health_identity::LockoutPolicy`.
- **Tokens:**
  - Access tokens are ES256 JWTs (`typ` `at+jwt`). Claims: `iss`, `sub`, `client_id`, `scope`, `sid`, `auth_time`, `amr`, `acr` (`central` or `offline-local`), and `cnf.jkt` when the token request carried a DPoP proof.
  - They expire after 15 minutes or at the session's 12-hour cap, whichever is first.
  - Refresh tokens rotate. Presenting a rotated-out token ends the session.
  - `openid` adds an ID token with `nonce`.
- **DPoP (RFC 9449):**
  - A proof must be ES256, carry a public JWK, match `htm` and `htu` (query ignored) and be no more than 300 s from our clock, and its `jti` is refused the second time.
  - On resource requests it must also carry `ath`, and its key must match the token's `jkt`.
- **Resource side:** `AccessVerifier::verify` checks the token, its binding, and that its session is still live, so logout takes effect at once. It returns a `Principal`. Authorization decisions are the policy decision point's ([#65](https://github.com/carapai/people-centered-healthcare/issues/65)).
- **Credentials:** Argon2id with m = 19456 KiB, t = 2, p = 1, plus an optional pepper (`Credentials`).
- **Breached passwords (#189):** `Credentials::set_password` is the check for every new password. It applies `PasswordPolicy` (8 characters with a device factor, 15 without) and refuses anything on `BreachedPasswords::platform()`.
  - The list is SecLists' top 1,000,000 (MIT), built into the binary as a 1.8 MB Bloom filter with 0.1% false positives, so central and edges check offline the same way.
  - Provenance and rebuilding are in `data/README.md`.
- **State:** `AuthStore` holds accounts, hashed codes, sessions with hashed refresh tokens, and the jti cache. `MemoryStore` serves tests and development.
- **`SurrealAuthStore`** persists that state in SurrealDB (migration `0008_auth`, owned by identity), one transaction per operation:
  - **Codes:** a code is taken with a single `DELETE … RETURN`, so two concurrent redemptions cannot both get it; the loser's commit conflicts.
  - **Refresh tokens:** rotation is a compare-and-set on the current hash, and the old hash is kept as retired, so a replay is recognised.
  - **Other state:** lockout counters, snapshots and the clock floor survive restarts.
  - **Provisioning:** `put_account` and `put_snapshot`.
  - **Purging:** `purge(now)` deletes expired codes and jtis, and sessions past their 12-hour cap, as a background job.
  - Only hashes of codes and refresh tokens are stored.

## Request subjects (ADR 0056)

`Subjects` and the `subject_layer` middleware turn each resource request into the `health_access_policy::Subject` and `health_audit::Agent` request extensions that `health-fhir-server` decides and audits with. The subject is built from:
- the verified access token;
- the current snapshot for the token's facility, re-read on every request, so a new or revoked snapshot applies at once;
- the `RolePermissions` mappings, through `health_identity_app::subject_of`;
- the `X-Purpose-Of-Use` header (`PURPOSE_HEADER`).

Refusals:
- an invalid token, or no current snapshot, is 401;
- an unknown purpose of use is 400;
- a request with no credentials at all passes without a subject, so the routes decide (the FHIR server serves only `/metadata` without one).

## Break-glass (offline-auth.md § Break-glass)

`Subjects::routes()` serves, under `/api` at central:
- **Requesting a grant:** `POST /break-glass` with `{"patient": "Patient/…", "reason": "BTG", "justification": "…"}` grants one patient for one hour (`BreakGlassPolicy`).
  - Refusals: 403 without the `break-glass` permission, 400 without the reason or a justification, and 409 while a grant is active.
  - The grant is stored for the session, with its AuditEvent (`Auditor::break_glass`) in the same transaction, and a privacy-officer review due in 72 h (`REVIEW_WITHIN`).
- **While a grant is active:** the session's `Subject` carries it and its audit `Agent` is flagged. Every access is then audited with `BTG`, and the PDP opens that patient's records.
- **Alerts:** `GET /break-glass/alerts` and `POST /break-glass/alerts/{id}/acknowledge` are for the facility in-charge, whose roles map to `ALERTS_PERMISSION` (`break-glass.alerts`).
- **Still to come:** pushing alerts to the in-charge, and the privacy officer's review ([#218](https://github.com/carapai/people-centered-healthcare/issues/218)).

## Permission snapshots (ADR 0011)

- **Signing:** `SnapshotSigner` is central's key. It signs a `PermissionSnapshot` as a compact ES256 JWS (`typ` `snapshot+jwt`), after checking its roles against the configured `RolePermissions`.
- **Verification:** `SnapshotVerifier` trusts central's snapshot keys, several of them during a rotation. Any changed byte, another key, or a token that is not a snapshot is refused.
- **The gate:** `AuthConfig::snapshots` takes a `SnapshotGate`. Then every login, code redemption and refresh needs the account's snapshot for the session's facility.
  - **The facility:** an edge's gate names its own facility. At central, `facility: None` means the user chooses at sign-in (ADR 0056). The form's `facility` field must name one of the account's snapshots (`AuthStore::facilities`). An account with one snapshot needs no `facility`. One with several and none named goes back to the sign-in page with `error=facility`. A facility the user holds no snapshot for is refused like a wrong password.
  - **Carried through:** the facility is kept on the code and the session (refreshes re-check that facility's snapshot) and in the token. `Principal` exposes `facility` and `snapshot_version`, and `/session` reports the facility.
  - **The checks, in order:**
    1. the local clock must not read before the floor;
    2. the snapshot must verify, and must be for this account and facility;
    3. it must be unexpired and not dated in the future;
    4. checking it advances the clock floor to its `issued_at`.
- **Failures:** a refused login shows the usual single message. A refused refresh ends the session.
- **Tokens:**
  - Access tokens carry `facility` and `snapshot_version`, and never outlive `not_after`.
  - The token response carries `snapshot_not_after`, so the client can warn in the last 3 days.
- **Storage:** `AuthStore::snapshot`, `facilities`, `clock_floor` and `set_clock_floor` default to none. `MemoryStore` and `SurrealAuthStore` keep them; `facilities` is a range scan over the `[account, facility]` record ids.
- **Offline administration** belongs with the edge's local authorization server ([#75](https://github.com/carapai/people-centered-healthcare/issues/75)):
  - the in-charge's single audited 7-day emergency extension, and district or central extensions;
  - local grants and provisional accounts, signed by the edge node key, lasting at most 7 days until central ratifies them;
  - never granting central-only roles offline;
  - applying central's revocations before uploading audit events and the outbox.

## Not here yet

- **Wiring:** central-server mounts it at `/auth` (#188). `TokenSigner::from_secrets` takes the active key first and rotated-out keys after it, which still verify. The web client's sign-in is the rest of [#188](https://github.com/carapai/people-centered-healthcare/issues/188).
- **Edge features:** PIN resume, and SMART Backend Services between edge and central (Phase 4).

## Tests

- **Unit:** JWS and DPoP verification and tampering; Argon2id parameters and the pepper.
- **`tests/flow.rs`**, through the routes:
  - login, then DPoP-bound access: replayed, foreign-key, stale and `ath`-less proofs are refused, as is a downgrade to Bearer;
  - refresh with rotation and key binding;
  - logout ending access at once;
  - a replayed refresh token ending the session;
  - single-use, PKCE-bound and expiring codes;
  - an edge refusing tokens without DPoP;
  - lockout after 10 failures, with a locked account indistinguishable from an unknown one;
  - the 12-hour cap;
  - unregistered redirects never followed.
- **`tests/surreal_store.rs`**, over SurrealDB:
  - the flow continues across a server restart mid-session: the access token still verifies, refresh rotates, and a replay ends the session;
  - logout;
  - lockout counters persist;
  - purge;
  - of four concurrent redemptions of one code, exactly one gets tokens (five rounds);
  - concurrent refreshes rotate at most once.
- **`tests/snapshots.rs`**, an edge with a `SnapshotGate`:
  - no snapshot, no login;
  - the snapshot's claims reach the token;
  - an expired snapshot blocks login and ends refresh, and the last token stops at `not_after`;
  - a renewed snapshot lets the user back in;
  - a tampered or impostor-signed snapshot is refused, as is one for another facility;
  - a clock wound back below the floor refuses logins.
