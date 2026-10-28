# Security, privacy and governance

This page consolidates how MASTER_PROMPT §95–97 is met. Each mechanism is decided in the linked document or ADR; this page adds no new decisions. Topology and trust boundaries are in [deployment-topology.md](deployment-topology.md).

## §95 coverage

| Requirement | How | Source |
|---|---|---|
| OIDC / OAuth2 | Central OIDC/SMART v2 authorization server; the edge runs the same Rust implementation for LAN clients. Desktops run it for their own window in mode B and in offline fallback | ADR 0010, desktop-client.md |
| RBAC | Roles via PractitionerRole for a facility and locations; role → permission mappings in the country configuration package | offline-auth.md |
| ABAC | Facility, location, program assignment, purpose of use, sensitivity labels, Consent scope, assurance level | offline-auth.md, ADR 0029 |
| Organization and facility scoping | Sync Scope, enforced by upstreams on push and pull; policy decision point (PDP) filters inside the database query | synchronization.md, offline-auth.md |
| Patient-level access | Caregiver access via RelatedPerson + Consent + verification; portal access at `level3` | patient-identity.md |
| Purpose of use | Declared for patient-sensitive access | offline-auth.md |
| Consent | Consent records (owner: consent); scoped, expiring caregiver grants | patient-identity.md |
| AuditEvent | Append-only at the origin node and synced up; only the audit module writes them | resource-ownership.md, write-authority.md |
| Provenance | Written with every derived write (extraction, identity survivorship, pricing versions) | questionnaire-extraction.md, patient-identity.md |
| Break-glass | One patient, 1 hour, reason code, immediate alert, privacy-officer review within 72 h | offline-auth.md |
| Session management | 15-minute DPoP access tokens with a 12-hour cap; PIN resume and switch; idle lock after 15 min; tokens in memory only | offline-auth.md, client-application.md |
| Device trust | Device-bound enrollment approved by a facility admin; DPoP keys; revocation | ADR 0009, lan-security.md |
| Encryption in transit | TLS everywhere: a private PKI with name-constrained facility intermediates on the LAN, mTLS for sync | ADR 0008, synchronization.md |
| Encryption at rest | Encrypted backups everywhere. Edges: mandatory full-disk encryption with TPM-only unlock, recovery keys escrowed at central. Desktops: full-disk encryption check (warn or block by configuration) | ADR 0048, ADR 0040, backup-and-recovery.md |
| Key management | See [Keys](#keys) | |
| Secure backups | Encrypted per-node, keys escrowed, drills | ADR 0046 |
| Logging and monitoring | OpenTelemetry to an in-country stack, with no personal health data | ADR 0047 |

Permissions are enforced in the Rust services on every node, never only in the UI. The PDP runs in the application layer, and searches receive its decision as a query filter (offline-auth.md).

## §96 Confidentiality

- **Sensitivity labels.** Records carrying `meta.security` labels (HIV, TB, mental health, SRH, SUD) or confidentiality `R` need a matching program assignment as well as the role (offline-auth.md).
- **The same policy applies everywhere:**

  | Surface | Mechanism |
  |---|---|
  | Clinical workspaces | PDP filter on every query and use case |
  | Patient portal | Assurance-gated access; released records only (patient-portal.md) |
  | Facility Edge Node | Same PDP, using signed permission snapshots offline |
  | Sync | Sync Scope; edges receive only their facility's records and golden records for their patients |
  | Analytics | Read models respect labels; the export rules are part of the analytics fog |
  | External integrations | Central adapters send only what each integration's contract requires (integration catalogue fog) |
  | Caregiver access | Adolescent sensitive records excluded from guardians from age 12 by default unless the adolescent consents |
  | Telemetry | Codes from sensitive ValueSets are never emitted (ADR 0047) |
  | Person links | Filtered by confidentiality, because the link itself can disclose (patient-identity.md) |

## §97 Facility isolation

- **Authority stays local.** Facility-scope records are authoritative at their facility and never edited elsewhere (ADR 0012).
- **Narrow sync.** Edges and desktops receive only their Sync Scope. Another facility's records are read through central's shared record, on demand and under the PDP (synchronization.md).
- **Tested.** The authorization-matrix test suite covers facility isolation (testing.md).

## Keys

| Key | Holder | Purpose | Rotation and custody | Source |
|---|---|---|---|---|
| National root | Offline; the platform operator until MoH governance is decided | Anchors the private PKI | Ceremony; custody in fog | ADR 0008 |
| Facility intermediate | Edge OS keystore | LAN TLS for the facility's names | 5 years; revoked centrally | ADR 0008 |
| Node and device keys | OS keystore (edge, desktop); non-extractable WebCrypto key (browser) | mTLS and DPoP | Re-enrollment; never recycled | ADRs 0009, 0013, 0040 |
| Edge pepper | Edge OS keystore (CNG machine key, or TPM where present) | Wraps the credential hashes | — | offline-auth.md |
| Permission-snapshot signing | Central | Signs per-user snapshots | — | ADR 0011 |
| Publication signing | Central HSM/KMS | Signs definition packages | Yearly | ADR 0024 |
| Release signing | Azure Artifact Signing, KMS, GPG, cosign | Installers, updater artifacts, edge packages, images | Per ADR 0045 | ADRs 0041, 0045 |
| Disk recovery keys (edge) | Wrapped and escrowed in central KMS; read out by phone to a verified facility admin; no on-site copy | Rotated after every use | ADR 0048 |
| Backup keys | Per node, wrapped and escrowed in central KMS; central master key under 3-of-5 Shamir | Backup encryption | Runbook custodians | ADR 0046 |

## Data protection (Uganda)

DPPA 2019 and its 2021 Regulations obligations (lawful basis, DPIA, registration, breach notification) and the question of PIN as MFA stay in the map's legal fog. The architecture assumes:
- all hosting and telemetry are in-country;
- only synthetic data is used in CI;
- breach response relies on the audit trail.

## Open items

- **Desktops hold credential hashes.** Mode B and offline-fallback desktops hold Argon2id hashes for the staff who may log in there, a wider exposure than ADR 0010 describes for the edge alone. They are mitigated by the keystore, the full-disk encryption check and revocation.
