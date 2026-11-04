# health-node-identity

Layer: **runtime**. A node's identity on its host, shared by the edge and the desktop. It is composed in their `apps/` roots, because a runtime library may not depend on another runtime library (dependency-rules.md rule 4).

## `encryption`: the full-disk encryption check (ADR 0048)

- **`probe(system, data, now)`** reads the host into an `EncryptionState`: TPM 2.0, Secure Boot, and the protection of the system volume and the data directory's volume. It is blocking, and needs administrator rights.
  - **Linux:**
    - `findmnt` names the device a path is mounted from, and `lsblk --inverse` walks from it to the disk. A `crypt` mapping over a `crypto_LUKS` device means the volume is encrypted, and `FSVER` gives the LUKS version.
    - `cryptsetup luksDump --dump-json-metadata` lists the LUKS2 tokens. A `systemd-tpm2` token is the TPM protector, and its `tpm2-pcrs` must include 7 (Secure Boot).
    - The TPM is read from `/sys/class/tpm/tpm0/tpm_version_major`, and Secure Boot from the `SecureBoot` EFI variable. Legacy BIOS boot means no Secure Boot.
  - **Windows:**
    - One PowerShell script reads `Win32_Tpm.SpecVersion`, `Confirm-SecureBootUEFI`, and `Get-BitLockerVolume` for each drive.
    - Compliant means `FullyEncrypted` with protection `On` and a `Tpm` protector. A PIN or startup key would need a person at every boot.
    - The drive letters are passed through the environment, never spliced into the script.
  - **Other platforms,** and anything that cannot be read, are `Protection::Unknown`.
  - The parsers (`parse_lsblk`, `parse_luks_dump`, `parse_secure_boot_var`, `parse_bitlocker`) are pure functions, tested against the tools' real output shapes.
- **`EncryptionState::findings`** lists every shortfall as a `Finding`. Each has an alert `code()`, and a `subject()` for the volume it concerns:
  - `tpm-missing` and `tpm-unknown`;
  - `secure-boot-off` and `secure-boot-unknown`;
  - `disk-unencrypted`, `tpm-protector-missing`, `encryption-suspended`, `tpm-not-sealed-to-secure-boot` and `encryption-unknown`, each for a volume.
  - Unknown counts against the machine, so the check fails closed.
- **`gate(state, exemption)`** decides provisioning:
  - `Allowed` when there are no findings;
  - `Exempt` with the grant when an `Exemption` covers them;
  - otherwise `Refused` with the findings.
- **`system_root()`** is `/`, or `%SystemDrive%\` on Windows.
- **`recovery`** handles disk recovery keys (ADR 0048 § Recovery keys; #268):
  - `new_recovery_password()` makes BitLocker's recovery-password format: 8 groups of 6 digits, each a multiple of 11 below 720,896. `valid_recovery_password` checks it.
  - **`RecoveryProtectors`** is the port that adds a key to a volume and removes one by its id, which is not secret. **`SystemProtectors`** is the host's:
    - **Windows:** `Add-BitLockerKeyProtector -RecoveryPasswordProtector` and `Remove-BitLockerKeyProtector`, with the key passed through the environment.
    - **Linux:** `cryptsetup luksAddKey`, with the key in a 0600 file under `/run`, removed at once. The new slot is found by comparing the header before and after (`parse_keyslots`). `cryptsetup luksKillSlot -q` removes a slot.

## `keystore`: sealed secrets

`Keystore::system()` seals and unseals named secrets, such as the facility intermediate's key. Secrets cross to the tools on standard input and output, never in arguments.

- **Linux:**
  - `seal` runs `systemd-creds encrypt --with-key=auto` as root, with the host key plus the TPM2 where present, into `/etc/credstore.encrypted/<name>`.
  - The service unit's `LoadCredentialEncrypted=<name>` makes systemd unseal it into `$CREDENTIALS_DIRECTORY`, which `unseal` reads first.
  - Root commands unseal with `systemd-creds decrypt`.
- **Windows:**
  - `seal` uses DPAPI in machine scope through PowerShell (`ProtectedData.Protect`, `LocalMachine`), into `%ProgramData%\health\credentials\<name>`.
  - The file's ACL is reset to SYSTEM and Administrators only.
  - The LocalSystem service unseals it with `ProtectedData.Unprotect`.
  - DPAPI binds the secret to the machine rather than the TPM chip; the BitLocker volume beneath it is TPM-bound. A CNG key would need FFI, and the workspace forbids `unsafe`.
- **Names and replacement:**
  - Names are plain credential names: letters, digits, `.`, `-` and `_`, not starting with `.`.
  - `seal` never replaces an existing secret (`KeystoreError::Exists`).

## Not yet

- **Other node keys:** the sync mTLS node key and the edge pepper use the same keystore when their features arrive.
- **The central side** (#268) is `health-provisioning`. The decommission record is still to come.
- **Exemptions:** an exemption is configuration (`HEALTH_ENCRYPTION_EXEMPTION`). Checking it against signed country configuration comes with configuration packages.

## Tests

`tests/encryption.rs`:
- **The parsers:** lsblk for Ubuntu's LUKS-over-LVM layout, plain disks, LUKS1 and plain dm-crypt. LUKS2 headers with a PCR 7 TPM token, an unbound one, and a passphrase only. The Secure Boot variable. BitLocker's compliant, suspended, PIN-only, decrypted, in-progress and failed volumes, PowerShell's single-element and null shapes, and TPM 1.2.
- **Findings:** every shortfall is its own finding and refuses provisioning. An exemption lets them through and names its grant.
- **The state file:** the state round-trips through the probe's file.
- **This machine:** the real probe reports it without failing.

`src/keystore.rs`:
- **Unsealed secrets:** the service reads what systemd unsealed.
- **No replacement:** a sealed secret is never replaced.
- **Names:** credential names are plain.
- **DPAPI (Windows):** seals and unseals in machine scope, with nothing readable at rest. The edge-build Windows job runs this.
