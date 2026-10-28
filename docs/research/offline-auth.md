# Research: offline authentication and authorization

This condenses the research ticket [How should offline authentication and authorization work for health workers?](https://github.com/carapai/people-centered-healthcare/issues/6). The full findings with citations are on the unmerged branch `research/offline-auth`. Decisions: [docs/architecture/offline-auth.md](../architecture/offline-auth.md), ADRs 0010 and 0011.

## OAuth 2.0, OIDC and SMART

- **RFC 9700 (OAuth Security BCP):**
  - The password grant "MUST NOT be used". Local password checking is a separate, non-OAuth authenticator.
  - Public clients MUST use PKCE.
  - Public-client refresh tokens must be sender-constrained or rotated.
  - Access tokens SHOULD be sender-constrained (mTLS or DPoP).
- **RFC 8252:** native apps (Tauri) MUST use an external user-agent, never an embedded webview login. Loopback redirects use the IP literal.
- **RFC 9449 (DPoP):** binds access tokens, and public-client refresh tokens, to a client key (`cnf.jkt`).
- **RFC 7009:** revocation does not reliably kill access tokens already issued. Offline, short token lifetimes matter more than revocation.
- **OIDC claims:** `auth_time`, `acr` and `amr` record when and how a user authenticated.
- **SMART App Launch 2.2.0:**
  - This version is R4-based, and no R5-specific release was found (unverified whether a newer one exists).
  - v2 scopes take the form `user/*.cruds`.
  - PKCE `S256` is required.
  - Access tokens SHOULD last ≤1 h.
  - "offline_access" means *the user is absent*, not *no network*.
- **SMART Backend Services:** the client is confidential and authenticates with a signed JWT. Tokens last 300 s, and refresh tokens SHOULD NOT be issued.

## Credential storage and authenticator policy

- **OWASP:** Argon2id is the first choice. `m=19456, t=2, p=1` is one of the equal-strength sets, and it is exactly the default of the `argon2` 0.6 crate.
- **NIST SP 800-63B-4:**
  - Stored passwords must resist offline attack. A pepper SHOULD be added, with its key stored separately, ideally in hardware.
  - Minimum length is ≥15 characters single-factor and ≥8 within MFA, so a short PIN is acceptable only as part of MFA (PIN + enrolled device key).
  - At most 100 consecutive failures.
  - AAL3 sessions: ≤12 h overall, ≤15 min inactivity.
- Hashes held on a stolen edge or laptop are open to offline cracking. Mitigations: Argon2id, a pepper in the keystore, encryption at rest, and provisioning only the facility's staff.

## Keystores

- **Windows:** DPAPI `LOCAL_MACHINE` blobs can be read by any local user, so a service should use CNG machine keys (LSA key isolation) or the TPM. Non-exportable keys via the TPM platform provider are unverified.
- **Linux:** Secret Service is a draft D-Bus API and probably needs a user session (unverified). For a headless edge, the candidates are the kernel keyring, systemd-creds or the TPM; none has been evaluated yet.
- **macOS:** Keychain details are unverified.
- **Crates:** `keyring` 4.2 (per-store crates via `keyring-core`), `jsonwebtoken` 11.1, `openidconnect` 4.0, `oauth2` 5.0, `biscuit-auth` 6.0 (attenuable capability tokens, a possible snapshot encoding), `zeroize` 1.9, `tauri-plugin-stronghold` 2.4.

## FHIR R5 security

- Security labels go in `meta.security`:
  - confidentiality, e.g. v3-Confidentiality `R`;
  - sensitivity, e.g. v3 InformationSensitivityPolicy `HIV`, `STD`, `SUD`;
  - PurposeOfUse.
- Break-glass is marked with `security-label#break-the-glass` and `v3-ActReason#BTG`. R5 requires implementations to be aware of break-glass but does not define the protocol, so the platform must.
- R5 AuditEvent carries PurposeOfUse in `authorization` and `agent.authorization`.

## Uganda DPPA 2019

| Section | What it says | Effect on design |
|---|---|---|
| s.6 | A data protection officer must be designated | |
| s.7(2)(d) | Processing without consent where necessary for medical purposes | |
| s.9 | Health data is special personal data | Whether s.7(2)(d) suffices without explicit consent needs counsel |
| s.8 | Children's data needs parent or guardian consent | Relevant to caregiver delegation |
| s.19 | Processing outside Uganda needs equivalent protection | A foreign-hosted IdP is affected |
| s.20 | Appropriate, regularly verified security measures | OWASP and NIST serve as the benchmark |
| s.23 | Notify the Authority "immediately" on suspected breach | No 72 h window appears in the Act |
| s.24(1)(c) | Data subjects may learn who accessed their data | Per-user audit, offline included, is mandatory |

The 2021 Regulations (S.I. 21/2021: DPIA, registration, breach notice) are unverified against the gazetted text.
