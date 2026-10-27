# Research: Offline authentication and authorization for health workers

Wayfinder research ticket: #6 (parent map #1, blocks #20). Spec anchors: `MASTER_PROMPT.md` §75, §76, §77, §83, §90, §91, §95–97.
Researched 2026-10-27. Every claim cites its source. Anything I could not confirm against a primary source is marked **unverified**.

---

## Summary (answer to the question)

Use **OIDC/OAuth 2.0 with central as the identity authority while the network is up, and a local verifier at the Facility Edge Node (and in standalone Tauri) when it is down.** The local verifier checks credentials against provisioned **Argon2id** hashes and issues **short-lived, Edge-signed session tokens**. Authorization works from a **signed, versioned permission snapshot** pushed down from central. Each snapshot has a hard `not_after` expiry, so §83's "no indefinitely valid cached permissions" holds.

Concretely:

1. **Online (central reachable).** Use the authorization code flow with PKCE through the system browser for Tauri, or the normal browser flow for web. Use SMART App Launch v2 scopes (`user/…cruds`, `fhirUser`) and sender-constrained (DPoP) tokens. Never use the password grant. Sources: RFC 9700 §2.1.1, §2.4, §2.2.1; RFC 8252 §5, §8.12; SMART App Launch 2.2.0.
2. **Edge as a local authority.** The Edge Node is a confidential client and resource server to central (SMART Backend Services, asymmetric JWT, ~5 min tokens). For LAN clients it acts as the local authorization server. It holds a device key in the OS keystore, enrolled with central.
3. **Offline login.** At provisioning, central pushes a per-user credential record (Argon2id PHC string, using the OWASP parameter set `m=19456 KiB, t=2, p=1`, which equals the `argon2` crate defaults) to Edge. The Edge verifies passwords and PINs locally, enforces lockout (NIST: at most 100 consecutive failures, a lower limit recommended), and issues Edge-signed access tokens of 15–60 min and session cookies.
4. **Cached authorization.** A **permission snapshot** (roles, facility and organization scope, sensitive-program grants, `issued_at`, `not_after`, `snapshot_version`, signature from central) governs offline authorization. When it expires, the user drops to a defined degraded mode, or is refused, per facility policy. Expiry is configurable but always finite.
5. **Shared workstations.** Use quick user switching with a short PIN re-auth, idle lock (≤15 min for clinical screens, in line with the NIST AAL3 inactivity SHOULD), and a hard session cap (≤12 h). Every action is attributed to the person, never to the device.
6. **Break-glass.** Allowed offline only for users whose snapshot carries a `break-glass` permission. It needs a reason (v3-ActReason `BTG` / PurposeOfUse), is time-boxed, is labelled with security labels, writes an `AuditEvent` immediately, and is flagged for mandatory review on sync.
7. **Revocation and reconnect.** On reconnect, the Edge pulls the revocation list and new snapshots *before* uploading the outbox, re-attests its device key, and re-evaluates offline actions taken by now-revoked users (they are flagged, not silently dropped). Device revocation removes the device's key/cert from Edge trust and wipes local credential caches.
8. **Legal (Uganda).** Health data is "special personal data" under DPPA 2019 s.9. Medical-purpose processing has a lawful basis under s.7(2)(d). s.20 requires "appropriate, reasonable, technical and organisational measures", s.23 requires *immediate* breach notification to the Authority, and s.24(1)(c) gives patients a right to learn who accessed their data. Together these make per-user attribution and audit of offline access a legal requirement, not just a nice-to-have.

---

## Findings by topic (with citations)

### 1. OAuth 2.0 / OIDC for native and offline-capable clients

- **Password grant is banned.** RFC 9700 (OAuth 2.0 Security BCP) §2.4: the resource owner password credentials grant "MUST NOT be used." This rules out "send the password to the Edge which proxies it to central" as an *OAuth* flow. Local password verification is a separate, non-OAuth authenticator. https://www.rfc-editor.org/rfc/rfc9700.html
- **PKCE is mandatory for public clients.** RFC 9700 §2.1.1: "Public clients MUST use PKCE." Implicit grant: "SHOULD NOT" (§2.1.2). https://www.rfc-editor.org/rfc/rfc9700.html
- **Refresh tokens for public clients** "MUST be sender-constrained or use refresh token rotation" (RFC 9700 §2.2.2). Sender-constraining of access tokens "SHOULD" use mTLS (RFC 8705) or DPoP (RFC 9449) (§2.2.1). https://www.rfc-editor.org/rfc/rfc9700.html
- **Native apps (Tauri)** "MUST use an external user-agent" (RFC 8252 §5) and "MUST NOT use embedded user-agents" (§8.12). Loopback redirect uses the IP literal, and `localhost` is NOT RECOMMENDED (§7.3, §8.3). A Tauri webview login page would violate this. https://www.rfc-editor.org/rfc/rfc8252.html
- **DPoP** binds access tokens, and for *public clients* also refresh tokens, to a client key pair, carried as `cnf.jkt` (RFC 9449 §1, §5, §6.1). This fits a device key held in the OS keystore. https://www.rfc-editor.org/rfc/rfc9449.html
- **OIDC `auth_time` / `max_age` / `acr` / `amr`** (OIDC Core 1.0 §2, §3.1.2.1) give a standard way to record *when* and *how* a user authenticated. Reuse these claim names in Edge-issued tokens, e.g. `amr: ["pwd"]` or `["pin"]` plus an `acr` value for "offline-local". https://openid.net/specs/openid-connect-core-1_0.html
- **Revocation.** RFC 7009 §2 defines a revocation endpoint. Revoking a refresh token should invalidate associated access tokens *if the server supports it*; otherwise they live until expiry (§2.1, §5). For offline nodes this means short access-token lifetimes matter more than revocation. https://www.rfc-editor.org/rfc/rfc7009.html

### 2. SMART App Launch v2 (scopes, launch context, backend services)

- Version fetched: **SMART App Launch 2.2.0 (STU 2.2), based on FHIR R4**. No R5-specific SMART release was found, so treat applicability to R5 as an adaptation (**unverified** whether a newer STU exists as of 2026-10-27). https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html
- **v2 scope syntax** is `patient|user|system/<Type|*>.<subset of cruds>`, e.g. `user/*.cruds`, `patient/Observation.rs`. Query-string constraints such as `?category=…` are allowed but marked largely experimental. The same page covers launch scopes (`launch`, `launch/patient`, `launch/encounter`) and `fhirUser`, which requires `openid` and yields a Practitioner/PractitionerRole/Patient/... URL. https://hl7.org/fhir/smart-app-launch/scopes-and-launch-context.html
- **`offline_access` vs `online_access`.** Both yield refresh tokens. `offline_access` stays valid "regardless of whether the end-user is online". Note that "offline" here means *the user is not present*, not *no network*. Same URL.
- **App launch rules.** "All SMART apps SHALL support PKCE", and servers SHALL support `S256` and SHALL NOT support `plain`. Access tokens "SHOULD have a valid lifetime no greater than one hour". A refresh token "SHALL be bound to the same client_id". https://hl7.org/fhir/smart-app-launch/app-launch.html
- **Backend Services** (fits the Edge-to-central sync identity). The client "SHALL be a 'confidential' client" authenticating with a signed JWT `client_assertion`. Recommended `expires_in` is 300 s, tokens "SHALL be short-lived", and "Refresh tokens SHOULD NOT be issued." https://hl7.org/fhir/smart-app-launch/backend-services.html

### 3. Local credential verification (Argon2id) and authenticator policy

- **OWASP Password Storage Cheat Sheet.** Argon2id is first choice. The equal-strength parameter sets are `m=47104,t=1,p=1`, `m=19456,t=2,p=1`, `m=12288,t=3,p=1`, `m=9216,t=4,p=1`, and `m=7168,t=5,p=1`. scrypt is the fallback, PBKDF2-HMAC-SHA256 with ≥600,000 iterations if FIPS is required, and bcrypt is legacy only. https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html
- **Rust `argon2` crate** (RustCrypto) is at **0.6.0** (crates.io, updated 2026-08-27). Argon2id is the default algorithm and output is PHC strings. Source shows `DEFAULT_M_COST = 19 * 1024`, `DEFAULT_T_COST = 2`, `DEFAULT_P_COST = 1`, which is exactly the OWASP `m=19456,t=2,p=1` set. https://docs.rs/argon2/latest/argon2/ ; https://github.com/RustCrypto/password-hashes/blob/master/argon2/src/params.rs ; https://crates.io/crates/argon2
- **NIST SP 800-63B-4** https://pages.nist.gov/800-63-4/sp800-63b.html
  - Verifiers "SHALL store passwords in a form that is resistant to offline attacks". The salt is ≥32 bits. The cost factor is "as high as practical". The verifier SHOULD add a keyed hash/encryption step (a pepper) with a separately stored key, ideally hardware-protected (§3.1.1.2). The pepper can come from the OS keystore or TPM on Edge.
  - Single-factor passwords must be ≥15 characters, and ≥8 when used within MFA (§3.1.1.2). Implication: a short clinical PIN is acceptable **only** as part of a multi-factor arrangement, e.g. PIN + enrolled device key, not alone.
  - Rate limiting: no more than 100 consecutive failed attempts, after which the authenticator is disabled; lower limits are allowed (§3.2.2).
  - Session limits. At AAL2 the overall timeout SHOULD be ≤24 h and inactivity ≤1 h (§2.2.3). At AAL3 the overall timeout SHALL be ≤12 h and inactivity SHOULD be ≤15 min (§2.3.3).
- **Offline-cracking risk.** Hashes pushed to an Edge box or laptop are exposed to offline attack if the disk is stolen. Mitigations are memory-hard Argon2id, a pepper in the OS keystore (NIST above), and encryption at rest, which is §95 of the spec.

### 4. Device-bound keys (OS keystores)

- **Windows DPAPI.** `CryptProtectData` binds data to the user's logon credential and "usually" the same computer. `CRYPTPROTECT_LOCAL_MACHINE` lets *any user on the machine* decrypt. The prompt-struct flow is deprecated, with removal in February 2027. For an Edge service running as a service account, LOCAL_MACHINE-scoped blobs are readable by any local user, so prefer CNG keys with key isolation or the TPM. https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- **Windows CNG.** Private keys go through the key storage router into the LSA key isolation process ("long-lived keys must be isolated so that they are never present in the application process"). Machine, local-service and network-service key directories are separate. https://learn.microsoft.com/en-us/windows/win32/seccng/key-storage-and-retrieval. TPM-backed "Microsoft Platform Crypto Provider" non-exportable keys: **unverified** (the KSP list page returned 404 when fetched).
- **macOS Keychain.** Apple's docs page is JS-rendered and could not be fetched. Accessibility classes (`kSecAttrAccessible...ThisDeviceOnly`), `SecAccessControl` biometry flags and Secure Enclave P-256 keys are from general knowledge and are **unverified** against https://developer.apple.com/documentation/security/keychain-services.
- **Linux Secret Service** is a freedesktop D-Bus API ("The Secret Service manages all the sessions and collections") and is still marked DRAFT. https://specifications.freedesktop.org/secret-service/latest/. That it depends on a user session bus, and so is unsuitable for a headless systemd Edge service, is **unverified** (inferred). For Linux Edge, consider the kernel keyring (`linux-keyutils` store), systemd-creds, or a TPM. Their fitness was not researched here.
- **Rust `keyring` crate 4.2.0** (crates.io, 2026-08-29) is split into store crates (`windows-native-keyring-store`, `apple-native-keyring-store`, `dbus-/zbus-secret-service-keyring-store`, `linux-keyutils-keyring-store`, `db-keystore`). The docs say apps "which want to control which credential stores they use" should use `keyring-core` with specific stores. https://docs.rs/keyring/latest/keyring/
- Other crates checked on crates.io (version / last update): `jsonwebtoken 11.1.0` (2026-09-16), `openidconnect 4.0.1` (2025-07-06), `oauth2 5.0.0` (2025-01-21), `biscuit-auth 6.0.0` (2025-07-16, offline-attenuable capability tokens, a possible fit for permission snapshots), `zeroize 1.9.1` (2026-10-06), `tauri-plugin-stronghold 2.4.0` (2026-10-14). https://crates.io/api/v1/crates/<name>. Suitability beyond the version check was not assessed.

### 5. FHIR R5 security labels, break-glass, audit

- **Security labels** go in `meta.security`. Confidentiality codes come from `http://terminology.hl7.org/CodeSystem/v3-Confidentiality` (e.g. `R` Restricted). Sensitivity examples include `HIV`, `STD`, `SUD` (v3 InformationSensitivityPolicy). PurposeOfUse comes from `http://terminology.hl7.org/CodeSystem/v3-PurposeOfUse`. The page shows a break-glass label `http://hl7.org/fhir/security-label#break-the-glass` and the request category `v3-ActReason#BTG`. https://hl7.org/fhir/R5/security-labels.html
- **R5 security page.** "Security implementations must be aware of the Break the Glass protocol" (§6.1.0.5.1). Patient-sensitive resources "often require a declared Purpose Of Use" (§6.1.0.2.4). The page does not define the protocol, so the platform must. https://hl7.org/fhir/R5/security.html
- **AuditEvent R5.** `AuditEvent.authorization` (renamed from `purposeOfEvent`) and `agent.authorization` (renamed from `purposeOfUse`) carry PurposeOfUse such as BTG. https://hl7.org/fhir/R5/auditevent.html. The `ETREAT` code was not confirmed on the fetched pages (**unverified** code name).

### 6. Uganda — Data Protection and Privacy Act 2019 (Cap. 97) and Regulations 2021

Act text taken from the ULII/Laws.Africa consolidated PDF, "Legislation as at 3 May 2019": https://media.ulii.org/media/legislation/18002/source_file/98f959926ba2429c/2019-9.pdf (HTML: https://ulii.org/akn/ug/act/2019/9/eng@2019-05-03; that page blocked automated fetch, so the PDF was used).

- **s.6.** The head of the institution "shall designate a person as the data protection officer".
- **s.7(2)(d).** Personal data may be collected or processed without prior consent where "necessary … for medical purposes".
- **s.9(1).** A person "shall not collect or process personal data which relates to … health status or medical records". Exceptions are in s.9(3) (employer obligations by law; consent "given freely"; non-profit bodies' members). Read with s.7(2)(d), medical-purpose processing has a basis. Whether s.7(2)(d) overrides s.9(1) without explicit consent is a **legal interpretation question, unverified**, and should go to counsel/PDPO.
- **s.8.** Children's data needs parent/guardian consent, legal necessity, or research/statistics. Relevant to caregiver-delegation features.
- **s.19.** Processing/storing outside Uganda requires equivalent protection or consent. This matters for any cloud IdP hosted abroad.
- **s.20(1)–(3).** "appropriate, reasonable, technical and organisational measures" against unlawful access. The controller must identify risks, maintain safeguards, "regularly verify" them, update them, and observe "generally accepted information security practices". This is the hook for citing OWASP/NIST as the benchmark.
- **s.23(1).** On belief of unauthorised access, notify the Authority "immediately". The Authority decides whether data subjects are notified (s.23(2)–(3)). Detecting unauthorised offline access depends on Edge audit logs surviving and syncing.
- **s.24(1)(c).** A data subject may request "the identity of a third party or a category of a third party who has or has had access". Per-user, per-record access audit, offline access included, is effectively mandatory.
- **s.35.** Unlawfully obtaining or disclosing personal data: fine ≤240 currency points or ≤10 years' imprisonment, or both.
- **Data Protection and Privacy Regulations, 2021 (S.I. 21 of 2021, gazetted 12 March 2021).** The primary text could not be fetched (ULII returned 403). Secondary sources say Reg. 12 requires a data protection impact assessment before processing, Reg. 13/15 cover registration with the PDPO, and Reg. 33 covers breach notification. **All unverified against the gazetted text.** Secondary sources: https://multilaw.com/Multilaw/Data_Protection_Laws_Guide/DataProtection_Guide_Uganda.aspx ; https://www.unwantedwitness.org/download/Data-Protection-Self-Assessment-Checklist.pdf ; https://www.dlapiperdataprotection.com/?c=UG&t=law. A "72-hour" breach window seen in one secondary source is **not** supported by the Act text ("immediately").

---

## Implications for the decision (#20 and the Wayfinder map item "Offline authentication")

### Options

**A. Edge as a local OIDC-compatible authorization server with signed permission snapshots (recommended).**
Central IdP (OIDC) → Edge enrolled as a confidential client (SMART Backend Services, device key in OS keystore/TPM) → Edge receives per-facility user records (Argon2id hashes + pepper-wrapped), signed permission snapshots with `not_after`, and a revocation list → LAN browser and Tauri clients authenticate *to the Edge* with an auth-code+PKCE flow hosted by the Edge (online or offline the same way) → Edge issues short-lived tokens whose claims embed `snapshot_version`, `auth_time`, `amr`, `acr=offline-local|central`.
- Pros: one login UX for LAN clients whether central is up or down; RFC 9700 compliant; all enforcement in trusted Rust (§95); audit at a single node.
- Cons: Edge becomes a high-value target holding hashes for all facility staff; needs careful key management and encrypted storage.

**B. Pass-through to central when online, local fallback only when offline.**
- Pros: central sees every login when reachable.
- Cons: two login paths and code paths; mode-switch bugs; the fallback is exercised rarely and so is under-tested. It still needs everything in A for the offline case.

**C. Long-lived central tokens cached on devices (no local verifier).**
- Contradicts §83 (indefinitely valid cached permissions), SMART's ≤1 h access-token guidance, and RFC 9700 refresh-token constraints. Breaks shared workstations. **Reject.**

**D. Capability tokens (e.g. Biscuit) issued by central, attenuated offline by Edge.**
- An interesting fit for the "signed snapshot that can only be narrowed" model, but niche. Rust support exists (`biscuit-auth 6.0.0`). Consider as an *implementation* of the snapshot in A, not a separate architecture. Needs a prototype.

**Standalone Tauri (mode B, §75)** is a single-user subset of A: the same snapshot, a local Argon2id verifier, and a device key in the user keystore. The snapshot is scoped to that device's provisioned users only.

### Parameters to fix in the decision

| Item | Proposed default | Basis |
|---|---|---|
| Hash | Argon2id m=19456 KiB, t=2, p=1 (PHC) + pepper in keystore | OWASP; argon2 crate defaults; NIST §3.1.1.2 |
| Password length | ≥15 chars single-factor; PIN only with device-bound factor | NIST §3.1.1.2 |
| Lockout | e.g. 5–10 failures → timed lock; hard cap ≤100 | NIST §3.2.2 |
| Edge access token | ≤15–60 min, DPoP-bound where the client can hold a key | SMART app-launch; RFC 9449 |
| Edge↔central | SMART Backend Services, 300 s tokens, no refresh | SMART backend-services |
| Idle lock (clinical) | ≤15 min, PIN to resume; hard session ≤12 h | NIST AAL3 §2.3.3 |
| Snapshot expiry | configurable per facility, e.g. 7–30 days, never infinite; warn ahead of expiry | §83; governance decision |
| Break-glass | role-gated, reason required, time-boxed (e.g. 1 h), AuditEvent with BTG, mandatory post-sync review | FHIR R5 security/AuditEvent |

### Risks

- **Stolen Edge or laptop** exposes hashes for many users (offline cracking) plus cached PHI. Mitigate with encryption at rest, a pepper in TPM/keystore, minimal cached user set, and remote revocation on reconnect.
- **Clock skew** on offline nodes breaks expiry checks. Needs monotonic-clock fallback and "last known central time" in the snapshot; not researched (**unverified** approach).
- **Revoked user keeps working offline** until the snapshot expires. This is inherent, and the expiry length is a governance trade-off: patient safety (staff locked out) vs. security.
- **Shared-PIN culture** on busy wards undermines s.24(1)(c) attribution. UX must make switching faster than sharing.
- **Linux headless keystore** is unsettled (Secret Service suitability unverified). Windows Edge as a service account needs CNG machine keys, not per-user DPAPI.
- **Break-glass abuse offline**, where no one reviews until sync. Mitigate with local supervisor alerting on the LAN.

### Open questions

1. Which central IdP (Keycloak, Ory, custom Rust)? Does it support DPoP and SMART v2 scopes natively? (Not researched.)
2. Snapshot expiry default and who may extend it: facility in-charge, district, or central only?
3. Is a PIN + enrolled-device combination acceptable as "MFA" for clinical users in the Uganda MoH context? Needs MoH/PDPO policy input.
4. Does processing without explicit consent rest on s.7(2)(d) alone, or must s.9(3) be satisfied? Legal opinion needed (**unverified**).
5. Exact obligations in the 2021 Regulations (DPIA, registration, breach-notice form) need verification against the gazetted S.I. 21/2021.
6. Is there a SMART App Launch release newer than 2.2.0, or R5-specific guidance, as of 2026-10-27? (**unverified**)
7. What is the TPM availability on typical Ugandan facility hardware? This determines whether TPM-backed keys can be required or only preferred.
