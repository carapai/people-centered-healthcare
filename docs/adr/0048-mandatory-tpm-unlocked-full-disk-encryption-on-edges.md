# Facility Edge Nodes require full-disk encryption with TPM-only unlock, so they still restart unattended

**Requirement.** Provisioning a Facility Edge Node is **blocked** unless the system volume and the volume holding the data directory are encrypted.
- **Windows:** BitLocker with a TPM protector.
- **Ubuntu:** LUKS2 with a TPM2 key slot from `systemd-cryptenroll`, sealed to the Secure Boot state (PCR 7).

**Exemption.** Country configuration may grant an audited exemption for a single facility, which shows as a standing alert at central.

**Hardware baseline.** Edge hardware must have TPM 2.0 and UEFI Secure Boot enabled.

**Recovery keys** are generated at provisioning and escrowed, wrapped, in central KMS next to the node's backup key (ADR 0046).
- **Retrieval:** an authorized central operations person reads the key to the facility admin by phone, after verifying identity against the device registry. Each retrieval is audited.
- **No printed copy** is kept on site.
- **Rotation:** the key is rotated after every use.

**Monitoring.** The edge reports its encryption state as a health indicator (ADR 0047). It alerts if protection is suspended, if the TPM protector is missing, or if the data directory is on an unencrypted volume. Suspensions made by the edge's own updater are resumed after the reboot.

**Backup volumes and USB backups** need no full-disk encryption, because backups are already encrypted by the application.

**Decommissioning** is a crypto-erase: clear the TPM or destroy the key slots, then reformat. It is recorded in the device registry together with the node revocation.

We chose this for three reasons:
- The edge holds a facility's authoritative records and every staff credential hash.
- SurrealDB has no encryption at rest.
- The edge must come back after a power cut with nobody present (facility-edge.md).

TPM-only unlock protects against a stolen or discarded disk. A stolen whole machine still needs an OS login, faces the TPM-backed pepper that protects the credential hashes, and is cut off by remote revocation.

## Considered Options

- **TPM plus PIN, or a USB startup key.** Rejected: a person would have to be present at every boot.
- **Network-bound unlock (Tang/Clevis).** Rejected: a facility has no second server on the LAN.
- **Warn-only, as for desktops (ADR 0040).** Rejected for edges: the exposure is the whole facility, not one replica.
- **Application-level encryption of SurrealDB data.** Rejected: there is no engine support, and it would break indexing.

## Consequences

- Older PCs without TPM 2.0 cannot be edges unless an exemption is granted.
- A BIOS or Secure Boot change can trigger recovery, so central operations must be reachable by phone.
