# health-auth

Layer: **infrastructure**. The OIDC / SMART App Launch v2 authorization server (offline-auth.md, ADR 0010). Central and every edge run this same code: central accepts bearer or DPoP tokens, and an edge requires DPoP (`AuthConfig::require_dpop`).

## Endpoints

`AuthServer::router()` provides these, relative to the issuer URL:

| Route | What it does |
|---|---|
| `GET /.well-known/openid-configuration`, `/.well-known/smart-configuration` | Server metadata: S256 PKCE only, public clients, ES256 for DPoP and ID tokens, SMART capabilities |
| `GET /jwks` | The ES256 public key; its `kid` is the RFC 7638 thumbprint |
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
- **State:** `AuthStore` holds accounts, hashed codes, sessions with hashed refresh tokens, and the jti cache. `MemoryStore` serves tests and development, and the SurrealDB adapter is [#187](https://github.com/carapai/people-centered-healthcare/issues/187).

## Not here yet

- **Wiring:** mounting the server in central-server, signing-key storage and rotation, and the web client's sign-in ([#188](https://github.com/carapai/people-centered-healthcare/issues/188)).
- **Snapshot claims:** `facility` and `snapshot_version` ([#64](https://github.com/carapai/people-centered-healthcare/issues/64)).
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
