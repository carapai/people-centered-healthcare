# central-server

Layer: **runtime**. Central server composition root (ADR 0001; service-catalogue.md). It runs one process over the shared `health-http` router, with a separate SurrealDB server.

## Configuration

| Variable | Default | |
|---|---|---|
| `HEALTH_BIND` | `0.0.0.0:8080` | Listen address |
| `HEALTH_SURREALDB_ENDPOINT` | required | e.g. `ws://surrealdb:8000` |
| `HEALTH_SURREALDB_USER` | `root` | |
| `HEALTH_SURREALDB_PASSWORD_FILE` | required | A file holding the password; the password is never read from the environment |
| `HEALTH_TENANT` | `ug` | SurrealDB namespace |
| `HEALTH_DATABASE` | `main` | |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | none | The in-country collector; without it only logs are written |
| `HEALTH_LOG` | `info` | `EnvFilter` directive |
| `HEALTH_AUTH_ISSUER` | none | The authorization server's issuer URL, e.g. `https://central.ug/auth`; without it nothing is mounted at `/auth` |
| `HEALTH_AUTH_KEYS_FILE` | required with an issuer | Signing keys, one hex-encoded 32-byte P-256 secret per line, the active key first. Keep a rotated-out key on a later line until its tokens have expired |
| `HEALTH_AUTH_PEPPER_FILE` | none | The pepper key: 64 hex characters (32 bytes) that every stored password hash is encrypted under (ADR 0059). Without it hashes are stored as they are, for development only |
| `HEALTH_AUTH_CLIENT_ID` | `health-client` | The web client |
| `HEALTH_AUTH_REDIRECT_URIS` | required with an issuer | The web client's redirect URIs, comma-separated |
| `HEALTH_AUTH_LOGIN_PAGE` | none | The web client's sign-in page, e.g. `/sign-in`; without it the authorization server shows its own minimal form |
| `HEALTH_AUTH_SNAPSHOT_KEYS_FILE` | none | Central's permission-snapshot signing keys, in the same format as the keys file. With it, each session acts for one facility, chosen at sign-in (ADR 0056) |
| `HEALTH_REGISTRY_NIN_SYSTEM` | none | The national ID identifier system. With it, `/fhir` serves `Patient/$match` and the client registry's reviewer operations (`Patient/$merge`, `$link`, `$unlink`), source Patients carry their blocking keys, and block counts are recomputed hourly. `/api/registry/reviews` serves the review queue to reviewers |
| `HEALTH_NODE_CODE` | `C1` | This node's short code in the numbers it mints. With the registry on, `/fhir/Patient/$register` mints MRNs as `{facility}-{node}-{sequence}` (ADR 0014) |
| `HEALTH_PORTAL_REDIRECT_URIS` | none | The patient portal's redirect URIs, comma-separated (#111). With it, a patient client signs patients in with a one-time code to their phone, and, with the registry and access policy, `/api/portal` serves self-registration |
| `HEALTH_PORTAL_CLIENT_ID` | `health-portal` | The patient portal's client |
| `HEALTH_PORTAL_COUNTRY_CODE` | `256` | The country code of national phone numbers (`0772…`) |
| `HEALTH_DEV_OTP_OUTBOX` | required with the portal | **Development and e2e only.** A file each one-time code is appended to (`{phone} {code}`); nothing reaches a phone. It is the only sender until an SMS provider is chosen ([#235](https://github.com/carapai/people-centered-healthcare/issues/235)), and central logs a warning while it is in use |
| `HEALTH_DEV_FHIR_SEED` | none | **Development and e2e only** (#92). A FHIR Bundle of definitional resources (Questionnaires, value sets, code systems, Libraries, plan and activity definitions), each with a UUID id and passing tier 1, written once at startup. A resource already stored is left as it is, and central logs a warning while the seed is in use. Activation ([#89](https://github.com/carapai/people-centered-healthcare/issues/89)) replaces it |
| `HEALTH_DEV_WORKFLOW_DEFINITIONS` | none | **Development and e2e only** (#99). A JSON file holding an array of Workflow Definitions that central runs at `/api/workflow`. Each is checked as at publication, and one that uses guards or typed actions (not wired yet) is refused, so central does not start. Central logs a warning while it is in use. Activation ([#89](https://github.com/carapai/people-centered-healthcare/issues/89)) replaces it with published packages |
| `HEALTH_PAYMENT_TILLS` | none | The tills, comma-separated (#254). With them and the access policy, `/api/payments` takes payments and runs cashier shifts |
| `HEALTH_MOMO_ACCOUNTS_FILE` | none | The facilities' MTN MoMo merchant accounts, a secrets file (#124): `{"base_url", "callback_url", "pending_minutes", "facilities": {"<facility>": {"subscription_key", "api_user", "api_key", "environment", "currency"}}}`. Needs `HEALTH_PAYMENT_TILLS`. With it, `/api/payments/momo` asks a payer's phone to pay into the session facility's account, `/api/payments/exceptions` lists the payment exceptions queue, MTN calls back at `/api/payments/momo/callback` (public; it only triggers a status read), and pending MoMo payments are polled every minute |
| `HEALTH_CURRENCY` | `UGX` | The ledger's functional currency (ISO 4217). Price Books and rules (#120), served at `/api/pricing` with the access policy, are in this currency |
| `HEALTH_ISSUING_CA_CERT_FILE`, `HEALTH_ISSUING_CA_KEY_FILE` | none | The facility issuing CA's certificate and PKCS#8 key, PEM, until its key moves to the KMS (#268). With them, `/api/provisioning/nodes` is served, so an edge can provision itself with a one-time code. Set both or neither |
| `HEALTH_KMS_KEY_FILE` | none | The master key, 64 hex characters, that wraps the edges' escrowed disk recovery keys until a KMS or HSM holds it (#268). With it, `/api/provisioning/recovery` is served and `central-server recovery` reads keys out |
| `HEALTH_FILES_DIR` | none | Central's blob store: edges upload synced attachments here and fetch them on demand (#84), and fetch the package tarballs activated for them (#89). `package` commands keep tarballs here. Without it, attachments and packages do not sync |
| `HEALTH_NATIONAL_ROOT_FILE` | none | The national root certificate (PEM) published packages are verified against. With `HEALTH_FILES_DIR`, central activates the national set every minute, writes its definitions into the store, and validates writes against its profiles and answers terminology from it (#89) |
| `HEALTH_BLOB_DIR` | none | Read by `package` commands only when `HEALTH_FILES_DIR` is not set (#88) |
| `HEALTH_PUBLICATION_KEY_FILE`, `HEALTH_PUBLICATION_CHAIN_FILE` | required by `package publish` | The publication-signing key (PKCS#8 PEM) and its certificate with any chain below the national root, until the key moves to the HSM or KMS |
| `HEALTH_ACCESS_POLICY_FILE` | none | The access policy as JSON: `{"roles": {"clinician": ["Patient.crs", …]}, "labels": [{"system": …, "code": "HIV", "program": "hiv"}]}`. With it and the snapshot keys, `/fhir` is mounted |

## Startup

1. Telemetry starts through the allowlist.
2. The listener starts at once: `/health` answers, and `/ready` is 503 until the database is ready.
3. The database is connected and migrated in the background, with capped exponential backoff (0.5 s to 30 s). Only an error kind is logged, never the message.
4. With an issuer configured, the authorization server (`health-auth` over `SurrealAuthStore`) is built once the database is ready, and `/fhir` with it when the snapshot keys and the access policy are configured. `/auth`, `/api/session` and `/fhir` answer 503 until then.

## Sign-in

- **Routes:** `/auth` serves discovery, `/jwks`, `/authorize`, `/token` and `/revoke` (offline-auth.md). `/api/session` is the first authorized route: it answers 401 without a valid access token, and the signed-in principal with one.
- **Key rotation:** add a new key as the first line of the keys file and keep the old one below it until its tokens have expired. Both keys are published in `/jwks`, and tokens signed with the old key still verify.
- **Key storage:** keys and the pepper are read from secret files. Holding them in central KMS is part of central's key custody ([#138](https://github.com/carapai/people-centered-healthcare/issues/138)).
- **Stock** (#130): with the access policy, `/api/stock` serves receiving, stock on hand, reorder suggestions, GS1 and requisitions to staff with `stock.manage`.
- **Caregivers** (#113): with the access policy, `/api/caregivers` serves staff with `caregiver.manage`, and `/api/portal/dependents` serves caregivers. The policy's labels are the sensitive ones an adolescent's guardian does not see.
- **Patients** (#111): with the portal configured, the portal's client signs patients in by phone (`/auth/otp`, then the code on `/authorize`; ADR 0029). `/api/portal/registration` sits behind `health_auth::patient_layer`, so only a patient's token reaches it.
- **The facility:** with snapshot keys, the user picks the facility at sign-in when they hold snapshots for several (ADR 0056). The token carries it.
- **`/fhir`** (ADR 0056):
  - Each request's subject is built from the access token, the current snapshot for the session's facility, the policy file's roles, and the `X-Purpose-Of-Use` header (default `TREAT`; an unknown code is 400).
  - Every interaction is decided at central's scope and audited.
  - A request without a token is 401, except `/fhir/metadata`.
  - Search parameters come with the configuration package. Until then the store's registry is empty, so `/fhir` serves reads, writes and history.
  - Facility-scope records carry the session's facility tag, and `_tag` is registered for every facility-scope type, so searches across patients stay within the facility (ADR 0057).
- **Break-glass:** `/api/break-glass` grants one patient for one hour to roles with `break-glass`, audited with `BTG`. The facility in-charge (`break-glass.alerts`) lists and acknowledges the alerts at `/api/break-glass/alerts`, and gets each new one at once over `/api/events`. The privacy officer (`break-glass.review`) works the queue at `/api/break-glass/reviews`. Overdue reviews are escalated hourly.
- **Snapshots** are provisioned with `SurrealAuthStore::put_snapshot` until the workforce registry issues them.

SIGTERM or Ctrl-C shuts the server down gracefully.

## Provisioning

`central-server provision <username> <facility> <role>[,<role>…]` creates or updates a Login Account and signs its permission snapshot at `facility`. It is the stopgap until the workforce registry provisions staff.

`central-server provision bundle <facility> <node-id> <intermediate.pem> <out>` exports a facility's provisioning bundle for one edge (ADR 0059):
- **What it holds:** every account with a snapshot at the facility, their signed snapshots, and each account's plain Argon2id hash.
- **Hashes:** central's pepper key unseals each one, and the bundle encrypts it to the edge's facility intermediate, the certificate central issued it.
- **Signature:** the bundle is signed with the snapshot key.
- **Output:** the command prints the intermediate's SPKI pin it encrypted to. The edge imports the bundle on its console (#75). The bundle also carries the access policy's `roles` and its `locally_grantable` list, the roles a facility admin may grant on an edge offline.

`central-server provision code <facility> --by <operator> [--dns <name>]… [--exempt <reason>]` issues a one-time code an edge provisions itself with (#268), and prints it once.
- **`--dns`:** the facility's DNS names, for its intermediate.
- **`--exempt`:** grants the edge an exemption from full-disk encryption (ADR 0048), with its reason. The exemption is audited, and central alerts on the node hourly while it is active.
- **Where the edge redeems it:** `POST /api/provisioning/nodes`, served when `HEALTH_ISSUING_CA_CERT_FILE` and `HEALTH_ISSUING_CA_KEY_FILE` are set ([health-provisioning](../../../crates/infrastructure/health-provisioning/README.md)).

`central-server recovery read <node-id> <system|data> --by <operator> --verified <how>` reads an edge's disk recovery key out, for an operator to give by phone to a facility admin whose identity they verified (ADR 0048).
- It prints the key once. The read is audited as a warning, with who read it and how the caller was verified.
- From then on the key is used: an hourly `recovery-key-used` alert until the edge escrows its replacement.
- `central-server recovery used` lists the used keys.
- No web endpoint returns a key.

`central-server node decommission <node-id> --by <operator> --erased <tpm-cleared|key-slots-destroyed> --reason <why>` records a decommissioned edge (ADR 0048), once the facility has crypto-erased it by clearing its TPM or destroying its key slots.
- In one audited transaction, its facility intermediate is revoked and its escrowed recovery keys are destroyed.
- The node is never active again.

`central-server node crl --out <file>` writes the issuing CA's next CRL, listing every decommissioned node's intermediate, for the edges. It needs the issuing CA settings.

`central-server provision snapshot-keys <out>` writes the snapshot public keys edges trust, `HEALTH_SNAPSHOT_KEYS_FILE` on an edge, as a JSON array of JWKs.

- It reads the same configuration as the server, and needs the auth settings, `HEALTH_AUTH_SNAPSHOT_KEYS_FILE` and `HEALTH_ACCESS_POLICY_FILE`.
- The password comes from `HEALTH_PROVISION_PASSWORD_FILE`. It must pass the password policy and the breached-password list (15 characters, since central sign-in has no device factor).
- The roles must be defined in the access policy.

## Seeds on removable media

`central-server seed export <facility> <file>` writes the facility's snapshot to `<file>` and its signature to `<file>.jws`, for an edge whose link cannot carry a first seed (#83; synchronization.md § Seeding). It signs with the publication signer (`HEALTH_PUBLICATION_KEY_FILE`, `HEALTH_PUBLICATION_CHAIN_FILE`). The edge applies it with `facility-edge seed import <file>`.

## Publishing packages

`central-server package …` runs the publication lifecycle for Published Definition packages (#88; configuration.md § Lifecycle; ADR 0025), until the Clinical Metadata Studio. `<id>` is `name@version`, and `--by` names who acted. Every step is audited.

| Command | What it does |
|---|---|
| `package draft <dir> --by <author>` | Starts or revises the draft of the package under `<dir>`, with `package.json` at its top. A published version is refused |
| `package submit <id> --by <author>` | An author submits it. Validation (rules, schemas, the package and artefact semver checks against the latest published version) and simulation run at once; a failure returns it to Draft and prints each problem |
| `package review <id> --by <reviewer>` | A reviewer who is not an author accepts it |
| `package approve <id> --by <approver>` | An approver who is not an author approves it: two distinct approvers for clinical content, one for configuration |
| `package request-changes <id> --by <person> --reason <why>` | Sends it back to Draft, clearing the review and approvals |
| `package publish <id> --by <operator>` | Signs the approved tarball, once every approval is in |
| `package status <id>` | Its stage, people and records, with any problems |
| `package list` | Every package not yet published |
| `package export <id> --out <dir>` | Writes a published package's `<name>-<version>.tgz` and `.tgz.sig` |

Simulation runs the package's workflow scenarios (`tests/` files of type `workflow-scenario`) in `WorkflowSandbox` (#101); any failing scenario returns the package to Draft.

## OpenAPI

`central-server openapi` prints central's OpenAPI document (the system routes and `GET /api/session`) and exits, without reading any configuration. The FHIR API is described by its CapabilityStatement. `tools/codegen` generates `@health/api-client` from it (`bun run generate`).

Central's FHIR search supports `_tag` on every facility-scope type, `status` on Questionnaire and QuestionnaireResponse for the forms screen (#92), `type`, `url` and `version` on Library, which completion finds Extraction Mappings by (#93), `family` and `given` on Patient, which reception finds a patient by (#102), and `patient` on the types the patient summary is drawn from (#104). `_security` is indexed on every facility-scope type, Patient and the summary's types: the search policy leaves out the records whose labels a subject may not see by `_security:not`. A node whose records were written before an index existed needs a reindex for it. Central completes submitted QuestionnaireResponses with extraction (#93). Activation (#89) replaces these with the SearchParameters of the active packages.
