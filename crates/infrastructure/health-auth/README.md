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
| `POST /otp` | Patient clients: sends a one-time code to the phone (#111) |
| `POST /token` | `authorization_code` (PKCE S256) and `refresh_token` grants |
| `POST /revoke` | Logout (RFC 7009): either token ends the session |

- **Requests:**
  - An unknown client or redirect URI gets an error page and is never redirected to. Other request errors are redirected back.
  - PKCE is required, and `plain` is refused. There is no password grant.
- **Sign-in:**
  - Every refusal shows one message, and an unknown username still spends one Argon2id verification, so responses reveal neither whether an account exists nor whether it is locked.
  - Lockout follows `health_identity::LockoutPolicy`.
- **Patients sign in by phone** (#111; ADR 0029), on clients registered with `Client::patient`, when `AuthConfig::phone_sign_in` is set:
  - `POST /otp` sends a code; `POST /authorize` takes the `phone` and `code`. Patients have no password, and a patient client never takes one, nor a staff client a phone.
  - Signing up and signing in are the same step, so no answer reveals whether a phone has an account. The first accepted code creates the phone's Login Account (`PhoneAccount`, `level1`); one account per phone, even when two first sign-ins race.
  - Codes follow `health_identity::OtpPolicy`: 6 digits, valid 5 minutes, 5 tries, at most one a minute and 5 an hour per phone. A code is stored only salted and hashed, compared in constant time, and accepted once.
  - Every try is a compare-and-set on the challenge's try count (`attempt_phone_otp`), so parallel guesses cannot share a try.
  - Codes leave through the `OtpSender` port. Until the SMS provider is chosen (#235), the only adapter is `DevOutbox`, which keeps them for a developer or an e2e test.
  - Patient sessions need no permission snapshot. Their tokens carry `amr: ["otp"]`, and `Principal::patient` is true. A phone account's id is refused by staff clients, and a staff account's by patient clients.
- **Tokens:**
  - Access tokens are ES256 JWTs (`typ` `at+jwt`). Claims: `iss`, `sub`, `client_id`, `scope`, `sid`, `auth_time`, `amr`, `acr` (`central` or `offline-local`), and `cnf.jkt` when the token request carried a DPoP proof.
  - They expire after 15 minutes or at the session's 12-hour cap, whichever is first.
  - Refresh tokens rotate. Presenting a rotated-out token ends the session.
  - `openid` adds an ID token with `nonce`.
- **DPoP (RFC 9449):**
  - A proof must be ES256, carry a public JWK, match `htm` and `htu` (query ignored) and be no more than 300 s from our clock, and its `jti` is refused the second time.
  - On resource requests it must also carry `ath`, and its key must match the token's `jkt`.
- **Resource side:** `AccessVerifier::verify` checks the token, its binding, and that its session is still live, so logout takes effect at once. It returns a `Principal`. Authorization decisions are the policy decision point's ([#65](https://github.com/carapai/people-centered-healthcare/issues/65)).
- **Credentials** (ADR 0059): plain Argon2id with m = 19456 KiB, t = 2, p = 1, so a hash is the same on every node, stored encrypted under the node's 32-byte pepper key (XChaCha20-Poly1305, the account id as associated data). A copied database yields only ciphertext, and a hash cannot be moved to another account. `Credentials::hash` makes the plain hash central provisions; `seal` and `open` move it in and out of storage; `verify` takes the account. Under a key a plain hash is refused, and without one a sealed hash is, so a key is never bypassed.
- **Breached passwords (#189):** `Credentials::set_password` is the check for every new password. It applies `PasswordPolicy` (8 characters with a device factor, 15 without) and refuses anything on `BreachedPasswords::platform()`.
  - The list is SecLists' top 1,000,000 (MIT), built into the binary as a 1.8 MB Bloom filter with 0.1% false positives, so central and edges check offline the same way.
  - Provenance and rebuilding are in `data/README.md`.
- **State:** `AuthStore` holds accounts, phone accounts and their code challenges, hashed codes, sessions with hashed refresh tokens, and the jti cache. `MemoryStore` serves tests and development.
- **`SurrealAuthStore`** persists that state in SurrealDB (migration `0008_auth`, owned by identity), one transaction per operation:
  - **Codes:** a code is taken with a single `DELETE … RETURN`, so two concurrent redemptions cannot both get it; the loser's commit conflicts.
  - **Refresh tokens:** rotation is a compare-and-set on the current hash, and the old hash is kept as retired, so a replay is recognised.
  - **Phone accounts and codes** (migration `0012_phone_sign_in`): a unique index on the phone, and a try counted only `WHERE attempts = $before`; of two concurrent tries, one commit conflicts and is refused.
  - **Other state:** lockout counters, snapshots and the clock floor survive restarts. The floor is written only when a snapshot advances it. Of two concurrent raises, the one that loses the write conflict refuses nothing (the floor only moves forward). Before this fix, overlapping requests could get a spurious 401.
  - **Provisioning:** `put_account` and `put_snapshot`.
  - **Purging:** `purge(now)` deletes expired codes and jtis, and sessions past their 12-hour cap, as a background job.
  - Only hashes of codes and refresh tokens are stored.

## Devices (#74)

`DeviceGate` enrolls LAN devices and keeps the device registry (lan-security.md § Device enrollment; ADR 0009). It is configured with `DeviceConfig`: the origins clients reach the node by, the enrollment rules, a 300 s DPoP window and a rate limit of 10 requests per source address per 10 minutes.

| Route | What it does |
|---|---|
| `POST /api/enrollment` | Open. A DPoP proof by the device's key, which proves it holds the key, and a `label`. Answers 201 with the request id, the pairing code to show and when it expires. 409 if the key is already an active device, 403 if it was revoked, 429 past the rate limit |
| `GET /api/enrollment` | Open. With a proof by the same key: `pending` (with the code), `enrolled` (with the device), `revoked`, or 404 |
| `GET /api/admin/enrollments` | The pending requests, without their codes: the admin types the code the device shows |
| `POST /api/admin/enrollments/approve` | `code`, `name` and `location`. 201 with the device; 404 for an unknown code, 410 for an expired request, 422 without a name or location |
| `GET /api/admin/devices` | Every device, revoked ones too |
| `POST /api/admin/devices/{id}/revoke` | Revokes a device; 409 if it already is |
| `GET /api/device` | Behind the gate: the calling device |

- **The gate:** `DeviceGate::layer(routes)` puts routes behind a check that each request carries a DPoP proof (RFC 9449) whose `htm` and `htu` match the request on one of the configured origins, at most 300 s from our clock and never seen before. Its key must be an enrolled, active device. Otherwise:
  - no proof, or a bad, stale or replayed one, is 401 with `WWW-Authenticate: DPoP`;
  - an unknown key is 403 `device-not-enrolled`;
  - a revoked device is 403 `device-revoked`, from its next request on.
  - Handlers find the device and the checked proof as an `EnrolledDevice` extension. Token verification (#75) must take the proof from there rather than check its `jti` again.
- **The admin routes are the caller's to guard.** The actor is a `DeviceAdmin` extension the guard inserts, or `edge-console` without one.
- **Audit:** an enrollment or a revocation commits with its AuditEvent (DICOM `110137`, coded `device-enrolled` or `device-revoked`, naming the device by `urn:health:device-id`). Refused enrollments and requests from unknown keys are logged with `alert = "enrollment-refused"` or `"unenrolled-device"`, so they reach the status page's telemetry and central, rather than writing a record per attempt.
- **State:** the `DeviceStore` port, on `MemoryStore` and on `SurrealAuthStore` (migration `0020_devices`). Approval takes the request and creates the device in one transaction, so a code is used once; revocation is a compare-and-set on the device's `version`.

## Provisioning bundles (ADR 0059)

`Bundle` carries a facility's Login Accounts and their signed snapshots from central to one edge.
- `seal(signer, recipient)` signs the bundle with central's snapshot key, as a JWS of type `health-bundle+jwt`.
- Each plain Argon2id hash is encrypted to the edge's facility intermediate public key: ECDH with a fresh P-256 key, HKDF-SHA-256 keyed on the node id, then XChaCha20-Poly1305 with the account id as associated data.
- `open(jws, verifier, node_id, recipient)` refuses a bundle with `BundleError` when:
  - it is not signed by a trusted snapshot key;
  - it is for another node;
  - its hashes do not decrypt with this node's intermediate key.
- The snapshots inside are verified at login, as every snapshot is.
- `SurrealAuthStore::snapshots_at(facility)` lists what a bundle carries.

## The edge's authorization server (#75)

- **Device-bound tokens** (ADR 0009): with `AuthConfig::devices`, `/token` issues and refreshes tokens only for a DPoP key that is an enrolled, active device. The check is `DeviceGate::check`. An unknown or revoked key gets 401 `invalid_dpop_proof`, so revoking a device ends its session within one access-token lifetime.
  - The check is at `/token`, not at `/authorize`: `/authorize` is a browser navigation, which carries no proof.
- **Idle lock and PIN** (`AuthConfig::idle_lock`; offline-auth.md § Tokens and sessions):
  - **Activity:** a session records its last activity at sign-in, at each refresh and at each unlock. Idle past `SessionPolicy::idle_lock` (15 minutes), a refresh gets 401 `session_locked`; the session lives on.
  - **`POST /pin`:** with a DPoP-bound access token, sets the user's 6-digit PIN on the device the token is bound to. It is stored per (account, device key), sealed under the pepper key like a password.
  - **`POST /unlock`:** `username`, `pin` and `client_id`, with a DPoP proof by the device's key. It resumes the user's live session on that device, or switches a shared device to it, and issues fresh tokens without an ID token: a PIN is not a sign-in.
    - Every failure gets one refusal, spending an Argon2 verification.
    - Five wrong PINs (`PinPolicy`) end that session and remove the PIN, so only a password signs the user in there again.
    - A PIN never starts a session.
  - **One session per user per device:** a new sign-in on a device ends the user's earlier session there.
- **One replay check per proof:** `DeviceGate`'s layer records a proof's `jti` only when no `Authorization` header comes with it. With a token, `AccessVerifier` records it. A gated route that does not verify the token therefore does not record the replay.
- **Importing the bundle** (ADR 0059): `Provisioning::router` serves `POST /api/admin/provision/accounts`, for the caller to guard. It takes the bundle's JWS as the body.
  - **What it refuses:** with 422, a bundle that is not central's, or is for another node or facility; a bundle encrypted to another key; and any snapshot inside that is not central's or not for one of the bundle's accounts at this facility.
  - **What it stores:** each hash re-encrypted under the edge's pepper key. An account already here keeps its failed-login state.
  - The node's intermediate key comes from a `RecipientKey`, unsealed from the keystore on a blocking thread.

## Offline administration (#75; ADR 0011)

`LocalAdmin` serves a signed-in facility admin on an edge (offline-auth.md § Offline user and role administration):
- **`GET /access/roles`** lists the roles the admin may grant here: those on the bundle's locally grantable list that carry no never-offline permission.
- **`GET /access/grants`** lists the facility's unexpired local grants. **`POST /access/grants`** takes `username`, `role` and `days`; the role must be on the bundle's locally grantable list and carry no never-offline permission (`health_identity::snapshot::NEVER_OFFLINE`), for 1 to 7 days, to staff already provisioned here.
- **Usernames:** grants and revocations are listed with the user's `username` where the edge knows it.
- **`GET /access/revocations`** lists local revocations. **`POST /access/revocations`** takes `username`. An admin cannot revoke their own access.
- **`POST /access/accounts`** creates a provisional account for new staff. It takes `username`, the admin's initial `password` (which must meet the policy), `role` and `days`, and creates the Login Account, its `ProvisionalAccount` record and the role's local grant in one transaction, audited. It answers 409 for a taken username.
  - **The first sign-in** with the admin's password is answered `change_password` (the login page's `error`, or 403 without one) until the user sets their own.
  - **The snapshot:** with no central snapshot, the account signs in under `ProvisionalAccount::snapshot`, which has no roles of its own. Its local grant gives it all it has, and it lapses with the grant.
- **`POST /password`** (any user, under `/auth`): `username`, the current `password` and a `new_password`, which must pass the password policy (8 characters with a device factor) and the breached-password list. A wrong current password counts towards lockout. It clears a provisional account's `must_change_password`.
- **Who may:** the caller's effective snapshot must give `access.administer-local` under the role mappings central provisioned in its bundle (`LocalConfig`). Otherwise 403.
- **Signing:** each grant is signed through a `GrantSigner` with the node's key, for central to ratify once sync exists.
- **Audit:** each grant and revocation commits with its AuditEvent (DICOM `110137`, coded `local-grant` or `local-revocation`).
- **Taking effect:** in `current_snapshot`, so at sign-in, refresh and every request:
  - a local revocation refuses the user at once;
  - unexpired grants add their roles (`with_grants`), never program assignments.
- **The bundle** carries `LocalConfig`, the role mappings and the grantable roles, and the import stores it. Central fills it from its access policy's `roles` and `locally_grantable`.

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

## Patient subjects (#111)

Patients have no permission snapshot, so the portal's routes sit behind `patient_layer` rather than `subject_layer`. It inserts a `health_access_policy::PatientSubject` (the phone account and its phone) for a patient's live token of an active phone account. A staff token is 403, an invalid one 401, and a request with no credentials passes without a subject.

## Caregivers (#113)

`CaregiverRoutes` serves caregiver delegation over `health_identity_app::caregivers`, auditing every version written.
- **Staff** (`caregiver_router`, behind `subject_layer`, needing `caregiver.manage`):
  - `GET /?patient=` lists a patient's caregivers.
  - `POST /` records one: the caregiver by the phone of their portal account (422 if the phone has none), the relationship, `guardian` or `delegated`, the scopes, the last day of access, and the document checked.
  - `POST /{id}/revoke` revokes a grant, and `POST /{id}/adolescent-consent` records the adolescent's consent, given or withdrawn.
- **Patients** (`dependents_router`, behind `patient_layer`): `GET /dependents` lists the people the signed-in caregiver cares for, with their names, scopes, expiry, and whether an adolescent's sensitive records are withheld.

## Break-glass (offline-auth.md § Break-glass)

`Subjects::routes()` serves, under `/api` at central:
- **Requesting a grant:** `POST /break-glass` with `{"patient": "Patient/…", "reason": "BTG", "justification": "…"}` grants one patient for one hour (`BreakGlassPolicy`).
  - Refusals: 403 without the `break-glass` permission, 400 without the reason or a justification, and 409 while a grant is active.
  - The grant is stored for the session, with its AuditEvent (`Auditor::break_glass`) in the same transaction, and a privacy-officer review due in 72 h (`REVIEW_WITHIN`).
- **While a grant is active:** the session's `Subject` carries it and its audit `Agent` is flagged. Every access is then audited with `BTG`, and the PDP opens that patient's records.
- **Alerts:** `GET /break-glass/alerts` and `POST /break-glass/alerts/{id}/acknowledge` are for the facility in-charge, whose roles map to `ALERTS_PERMISSION` (`break-glass.alerts`).
- **Pushing alerts:** each grant sends a `break-glass-alerts` hint to the facility's in-charge and a `break-glass-reviews` hint to the privacy officers. `GET /events` streams each caller the hints for their facility and permissions.
- **Review:** the privacy officer (`break-glass.review`) lists pending reviews, the earliest due first and flagged when overdue, at `GET /break-glass/reviews`.
  - `POST /break-glass/reviews/{id}` records `appropriate` or `inappropriate` (with a required note). The decision and its AuditEvent commit in one transaction, and a review is made once.
  - `Subjects::escalate_overdue` marks each overdue review escalated once and tells the reviewers.
- **Still to come:** the web client's screens, and the review clock for grants made at an edge, counted from their arrival at central with sync ([#218](https://github.com/carapai/people-centered-healthcare/issues/218)).

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
