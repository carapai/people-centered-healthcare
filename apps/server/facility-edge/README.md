# facility-edge

Layer: **runtime**. The Facility Edge Node's composition root (ADR 0007, [facility-edge.md](../../../docs/architecture/facility-edge.md)): one OS service, one process, embedding SurrealDB. [`health-edge-runtime`](../../../crates/edge/health-edge-runtime/README.md) runs the node.

Deploying an edge to a facility waits for the power-loss spike, [Does embedded SurrealDB RocksDB survive hard power loss on facility hardware?](https://github.com/carapai/people-centered-healthcare/issues/34) (#34), which now applies to SurrealKV, the embedded engine since ADR 0060. The binary is built on the embedded adapter now. If the spike fails, only the engine settings change.

## Modes

- `facility-edge`: foreground, for development in a terminal. Ctrl-C stops it.
- `facility-edge --service`: under the service manager. On Linux this is systemd, `Type=notify`, with [`packaging/facility-edge.service`](packaging/facility-edge.service). On Windows it is the Service Control Manager: [`packaging/install-service.ps1`](packaging/install-service.ps1) registers the service, its environment and the failure actions (restart three times, then stay stopped; the count resets after 24 hours).
- `facility-edge update …`: signed updates (below).
- `facility-edge backup …`: backups on demand, to USB, verification and restore (below).

## Configuration

| Variable | Default | |
|---|---|---|
| `HEALTH_DATA_DIR` | required | The embedded store's directory. This process owns it alone |
| `HEALTH_BIND` | `0.0.0.0:443` | The LAN listener: HTTPS once provisioned, nothing before that (see TLS) |
| `HEALTH_CONSOLE_BIND` | `127.0.0.1:8080` | The console for the edge's own screen: plain HTTP, loopback addresses only, never `HEALTH_BIND`'s address and port. The admin pages answer here until staff sign-in (#75) |
| `HEALTH_TENANT` | `ug` | SurrealDB namespace |
| `HEALTH_DATABASE` | `main` | |
| `HEALTH_UPS` | none | The NUT UPS to watch, `name@host[:port]` (port 3493 by default). Without it, durability rests on `sync=every` |
| `HEALTH_UPS_POLL_SECS` | `5` | |
| `HEALTH_DRAIN_SECS` | `20` | How long in-flight requests may take to finish on a stop |
| `HEALTH_UPDATE_DIR` | `updates` beside `HEALTH_DATA_DIR` | Staged packages, the previous binary and the pre-update store copy. Keep it on the store's volume |
| `HEALTH_UPDATE_KEY_FILE` | required to stage | The pinned release key: a P-256 public key, hex SEC1 |
| `HEALTH_NODE_ID` | `facility-edge` | The node name recorded in its backups |
| `HEALTH_BACKUP_DIR` | none | Backups, on the second volume. Without it the node takes none and warns at every start |
| `HEALTH_BACKUP_KEY_FILE` | required with `HEALTH_BACKUP_DIR` | The node's backup key, 64 hex characters. Escrow at central is #138 |
| `HEALTH_BACKUP_INTERVAL_SECS` | `3600` | |
| `HEALTH_BACKUP_TEST_SECS` | `604800` | How often the newest backup is test-restored |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | none | Without it only logs are written |
| `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` | none | The in-country collector's OTLP/HTTP logs endpoint. Without it, log lines stay in the local buffer |
| `HEALTH_TELEMETRY_DIR` | `telemetry` beside `HEALTH_DATA_DIR` | The bounded buffer (64 MiB) of log lines waiting to be shipped |
| `HEALTH_TELEMETRY_SHIP_SECS` | `30` | How often the buffer is shipped |
| `HEALTH_INDICATORS_SECS` | `60` | How often the indicators are logged and alerts raised |
| `HEALTH_ENCRYPTION_STATE_FILE` | none; the unit sets `/run/health/encryption.json` | The privileged probe's result. Without it, the node probes the host itself, which needs administrator rights (the Windows service runs as LocalSystem) |
| `HEALTH_ENCRYPTION_EXEMPTION` | none | The country-configuration grant exempting this facility from full-disk encryption (ADR 0048): a standing alert |
| `HEALTH_ENCRYPTION_SECS` | `3600` | How often disk encryption is checked |
| `HEALTH_PKI_DIR` | `pki` beside `HEALTH_DATA_DIR` | The installed national root and facility intermediate chain. The intermediate's key is in the keystore, never here |
| `HEALTH_ORIGINS` | `https://<node-id>.local` | The origins LAN devices reach the node by, comma-separated. A device's DPoP proofs must name one of them (#74) |
| `HEALTH_TLS_DIR` | `tls` beside `HEALTH_DATA_DIR` | The server leaf and its key (`leaf.pem`, `leaf.key`, the key readable by the service alone), issued and renewed by the node |
| `HEALTH_DNS_NAME` | none | The facility's configured DNS name, named in the leaf beside `<node-id>.local` |
| `HEALTH_CERTIFICATE_SECS` | `3600` | How often the leaf is checked for renewal and address changes |
| `HEALTH_MDNS` | `on` | `off` stops the `_health-edge._tcp` mDNS announcement |
| `HEALTH_FACILITY` | none | The facility this edge serves. With `HEALTH_SNAPSHOT_KEYS_FILE`, the node serves staff sign-in (#75) |
| `HEALTH_SNAPSHOT_KEYS_FILE` | none | Central's snapshot public keys, a JSON array of JWKs (`central-server provision snapshot-keys`) |
| `HEALTH_LOG` | `info` | `EnvFilter` directive |

## Web app

The binary embeds the client and serves it, with its `runtime.json` and app-shell service worker, from its own origin (#76). Build the client first: `bun run build` in `apps/client`, then `cargo build -p facility-edge`. A binary built without `apps/client/dist` serves only its API. `bun run test:e2e:edge` in `apps/client` runs the browser suite against the built binary.

## Exit codes

| Code | Meaning | systemd |
|---|---|---|
| 0 | Stopped on request or on UPS low battery | not restarted |
| 1 | The listener, telemetry or serving failed | restarted |
| 2 | The configuration is missing or malformed | not restarted (`RestartPreventExitStatus=2`) |
| 3 | The store cannot be opened: another process holds it, or the disk failed | restarted |
| 4 | A migration failed, or a newer binary migrated the store | restarted; the update rollback (part 2) acts on it |
| 5 | A supervised task kept failing | restarted |
| 6 | A failed update was rolled back | restarted, which runs the previous binary |
| 7 | An update step failed, or an `update` command was refused | not restarted |
| 8 | A failed update's store is restored, and the binary is left for the service manager | not restarted; `OnFailure=` runs `packaging/facility-edge-rollback.service` |
| 9 | A `backup` command failed | (commands only) |
| 10 | The encryption gate refused provisioning (`encryption check`, `provision request`), or an `encryption` command failed | (commands only) |
| 11 | A `provision` command failed | (commands only) |

The first start and three restarts are allowed within 24 hours. After that the unit stays stopped.

On Windows a non-zero code is reported to the SCM as a service-specific error, and the failure actions restart it whatever the code. Exits 2, 7 and 8 therefore retry three times before the service stays stopped. The service runs as LocalSystem and can replace its own binary, so a rollback finishes in the process (exit 6), and the rollback unit's step is not needed. Long migrations and drains advance the SCM checkpoint with a wait hint, so the SCM keeps waiting.

## Updates

A package is a gzipped tar holding `manifest.json` (`version`, `binary`, `sha256`) and the binary. It comes with a detached ECDSA P-256 signature over the whole package, as hex `r || s`. `health_edge_runtime::update::pack` builds one; the release job signs it.

1. `facility-edge update stage <package> <signature>`: verifies the signature against the pinned key and the binary against its digest, then unpacks into `staged/`. The node may keep running.
2. `facility-edge update apply`, with the service stopped (`systemctl stop`): keeps this binary in `previous/`, swaps in the staged one, and journals the update as pending. Then start the service.
3. **The new binary's first start** copies the store to `pre-update-store/` before opening it, then migrates. Ready commits the update.
4. **A failed update rolls back** when a migration fails, or after three starts that never reached ready:
   - the store comes back first (the failed one is kept in `failed-store/`), then the previous binary, so an old binary never runs against a migrated store;
   - the sandboxed service may not write `/opt/health/bin`, so it restores the store and exits 8;
   - `facility-edge-rollback.service` then runs `previous/facility-edge update rollback /opt/health/bin/facility-edge` as root and starts the service again.
5. `facility-edge update status` prints the journal and anything staged.

Updates install in a maintenance window, or when an admin approves with no active sessions. That decision belongs to whoever runs `apply`; the session check comes with the local authorization server (#75), and pulling packages from central comes with sync.

## Backups

The running node takes a backup every hour and test-restores the newest one every week (`health_edge_runtime::backup`). Both are recorded in `status.json` under `HEALTH_BACKUP_DIR`. A failure is logged as an alert and never stops the node. The pre-update backup is the store copy an update takes.

- `facility-edge backup now`: takes a backup. The node owns its store, so stop the service first.
- `facility-edge backup copy <dir>`: copies the newest backup and its blobs to `dir` (a USB drive), then verifies the copy. The node may keep running.
- `facility-edge backup verify`: checks every backup's checksums, decryption and blobs.
- `facility-edge backup test`: the weekly test restore, now.
- `facility-edge backup restore <backup>`: with the service stopped and the old data directory moved aside, restores `backup` into a new store at `HEALTH_DATA_DIR`. It refuses a directory that holds anything.
- `facility-edge backup status`: prints `status.json` and the backups kept.

A backup on demand while the node runs is not available yet: the admin page (#77) shows the backup status only.

## Provisioning

The edge's half of provisioning (lan-security.md; ADRs 0048, 0058; #268). Central's half is `health-provisioning`. Recovery-key escrow is still to come.

**Online, with a one-time code** (the usual way): `facility-edge provision --code XXXX-XXXX --central https://central… --root FILE [--central-ca FILE]`, as root or Administrator, with the national root from the installation media.
1. It runs the encryption gate. An edge that fails it reports itself unencrypted, and central accepts that only if the operator exempted the code (`central-server provision code --exempt`). Otherwise it exits 10.
2. It seals the intermediate's key, and the pepper and token keys, as `provision request` does. A key left by an attempt that did not finish is reused. A machine already provisioned is refused, since a re-provisioned machine is a new node.
3. It posts the request to `POST /api/provisioning/nodes` over HTTPS.
   - Central's certificate is verified against the system trust store, or against `--central-ca` alone for an in-country private CA.
   - There is no bundled root store, and verification cannot be turned off.
4. It installs central's chain as `provision install` does, for the node ID central minted.
5. It writes `node.json` (node ID, short code, facility, and central's address and CA) to `HEALTH_PKI_DIR`.
   - The node runs under them unless `HEALTH_NODE_ID` or `HEALTH_FACILITY` is set.
   - `provision status` shows the short code.
6. On an encrypted edge, it generates and escrows the disk recovery keys, as `encryption rotate-recovery` does. On Linux, cryptsetup asks for the install passphrase. If escrow fails, the node is provisioned but exits 11, saying to run `encryption rotate-recovery`.

A wrong or used code exits 11. So does an unreachable or untrusted central, and the code stays redeemable.

**Disk recovery keys** (ADR 0048): `facility-edge encryption rotate-recovery [--central URL] [--central-ca FILE]`, as root or Administrator. Run it after central reads a key out (`central-server recovery read`). Central's address defaults to the one in `node.json`.
1. For the system volume, and the data directory's when it is another one, it generates a recovery password: 48 digits in 8 groups of 6, as BitLocker uses, readable over the phone.
2. It adds the password to the volume.
   - **Windows:** `Add-BitLockerKeyProtector`.
   - **Linux:** `cryptsetup luksAddKey`, which asks for a secret the volume already has. That is the install passphrase, or the key just read out.
3. It escrows them all in one JWS signed with the facility intermediate's key (`POST /api/provisioning/recovery`). If central does not take them, the new keys are removed again, so the disk never holds a key central does not.
4. It records the new keys' ids in `recovery.json` (ids only, never a key).
5. It removes the keys it escrowed before. Keys it did not create, such as the install passphrase, are left alone.

These commands need an encrypted volume with a TPM. They are tested with a stand-in for the platform tools, and are to be confirmed on facility hardware.

**By files** (no connectivity at the edge):

1. **`facility-edge provision request --out FILE`** runs as root or Administrator.
   - The encryption gate comes first: it exits 10 unless the machine complies or `HEALTH_ENCRYPTION_EXEMPTION` exempts it.
   - It then generates the facility intermediate's P-256 key and seals it as `health.facility-intermediate.key`, which is never replaced. A re-provisioned machine is a new node.
   - Last, it writes the request for central's issuing CA.
2. **Central issues the intermediate** from the request. Until #268, that is `tools/pki facility`.
3. **`facility-edge provision install --chain FILE --root FILE`** takes the chain (the intermediate, then the issuing CA) and the national root.
   - It unseals the key and checks that the certificate is a facility intermediate for it.
   - It issues a throwaway leaf for `<HEALTH_NODE_ID>.local` and verifies it against the root.
   - Only then does it install both under `HEALTH_PKI_DIR`.
4. **`facility-edge provision status`** prints what is sealed and installed, and when the intermediate expires.

**The keystore** (`health_node_identity::keystore`):
- **Linux:** `systemd-creds encrypt --with-key=auto`, with the host key plus the TPM2 where present, into `/etc/credstore.encrypted/`. The unit's `LoadCredentialEncrypted=health.facility-intermediate.key` unseals it into the unprivileged service's `$CREDENTIALS_DIRECTORY`. Before provisioning it is absent, which is not fatal.
- **Windows:** DPAPI in machine scope through PowerShell, into `%ProgramData%\health\credentials\`, readable by SYSTEM and Administrators only. The LocalSystem service unseals it itself.

**At each start** the node logs `identity-loaded`, or a `not-provisioned` or `identity-invalid` alert. Restart the service after `provision install` so it starts serving TLS.

## TLS

Once provisioned, the node serves HTTPS only, with a server leaf it issues itself under its facility intermediate (#74; lan-security.md § Certificate hierarchy):
- **The leaf:** ECDSA P-256 with serverAuth, valid 397 days. It names `<node-id>.local`, `HEALTH_DNS_NAME` and the node's private LAN addresses: the bound address if it is a specific private one, else every RFC 1918 or ULA address on its interfaces. A client trusting the national root accepts it.
- **Its key** is new at each issue. It is kept in `HEALTH_TLS_DIR` and readable by the service alone. The intermediate's key is unsealed from the keystore only to issue.
- **Renewal:** every `HEALTH_CERTIFICATE_SECS` the leaf is reissued if two-thirds of its life has passed or the addresses changed (`address-changed`, standing for a day). It is served from the next handshake, with no restart. If renewal fails, the current leaf is kept while it is valid, with a `certificate-renewal-failed` alert.
- **Expiry alerts:** `certificate-expiring` under 30 days. `intermediate-expiring` under 400 days, because a leaf cannot outlive its intermediate, so re-provision before then.
- **The console** on `HEALTH_CONSOLE_BIND` serves plain HTTP on the loopback address, provisioned or not. Browsers treat `http://localhost` as a secure context, and the admin pages, which answer loopback peers only, work on the edge's own screen with no certificate warning.
- **Before provisioning,** or if no leaf can be served (`identity-invalid`), the node opens no LAN listener, so nothing on the LAN talks to it in the clear. The console and `provision` still work on the machine itself.
- **Linux:** the unit grants `CAP_NET_BIND_SERVICE` so the unprivileged service can bind 443.

## Devices

Every `/api/…` route except enrollment and the status page needs a DPoP proof by an enrolled, active device (#74; lan-security.md § Device enrollment). A device asks with `POST /api/enrollment` and shows the pairing code it gets back. On the node's own console, an admin approves it with `POST /api/admin/enrollments/approve` (the code, a name and a location) or revokes it with `POST /api/admin/devices/{id}/revoke`. Requests expire after 10 minutes. The routes are described in `health-auth`'s README; the client screens are #74 part 3.

## Staff sign-in

The edge runs the same authorization server as central (offline-auth.md; #75), so staff sign in on an enrolled device with no internet.
- **Keys:** `provision request` seals the pepper key (ADR 0059) and the access-token signing key beside the intermediate's, and the unit unseals all three for the service.
- **Accounts:** central exports a facility's staff in a signed bundle (`central-server provision bundle`), and the edge's own screen imports it with `POST /api/admin/provision/accounts`. Each hash is re-encrypted under the edge's pepper key.
- **Sign-in:**
  - `/auth` is the authorization code flow with PKCE;
  - tokens are DPoP-bound, `acr` `offline-local`, and issued only to an enrolled device's key;
  - each login and refresh needs the user's central-signed snapshot for `HEALTH_FACILITY`, unexpired, at a clock that has not gone back;
  - `/api/session` is behind the device gate;
  - `runtime.json` gives the web client the issuer: the first origin in `HEALTH_ORIGINS`, under `/auth`, so list the https origin LAN devices use first.
- **Without** `HEALTH_FACILITY`, `HEALTH_SNAPSHOT_KEYS_FILE` or the sealed keys, the node serves no sign-in and logs `sign-in-off` with the reason.

## Pairing and discovery

Devices find and pin the edge by pairing, with mDNS as a hint only (#74; lan-security.md § Addressing and discovery):
- **Pairing:** the status page on the edge's own screen shows a QR code and its text, from `GET /api/admin/pairing` (console only). It carries:
  - the first `https` origin in `HEALTH_ORIGINS`;
  - the node ID;
  - the facility intermediate's SPKI fingerprint (`sha256/<base64>`), as `health-edge://pair?v=1&node=…&url=…&spki=…`.
  Before provisioning it answers 409 `not-provisioned`.
- **mDNS:** while the node serves HTTPS, it announces `<node-id>._health-edge._tcp.local.` on `HEALTH_BIND`'s port, at its leaf's addresses (every interface's when it names none), with TXT `node` and `v`. It re-announces when the addresses change. Clients still check the node ID and the fingerprint, so a spoofed announcement gets them nowhere. The host firewall must allow UDP 5353 on the LAN.

## Health monitoring

The node measures its indicators and raises alerts against the thresholds in facility-edge.md § Health monitoring (`health_edge_runtime::admin`).

- **Status page:** `/edge/status` in the embedded app, from `GET /api/admin/status`. Until edge sign-in arrives (#75), the node serves the report to a browser on the node itself only (403 elsewhere).
- **Telemetry:** every log line goes to standard output and to the buffer under `HEALTH_TELEMETRY_DIR`, including an `indicators` line every `HEALTH_INDICATORS_SECS` and each alert as it is raised and cleared. The buffer is shipped to `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT` when the collector answers, so indicators recorded offline reach central once the link is back. When the buffer is full, the oldest lines are dropped first.
- **Disk encryption** (ADR 0048, `health-node-identity`):
  - `facility-edge encryption probe [--out FILE]` reads the host's TPM, Secure Boot and the protection of the system and data volumes, as JSON.
  - `facility-edge encryption check` is the provisioning gate. It exits 10 unless the machine complies or `HEALTH_ENCRYPTION_EXEMPTION` exempts it.
  - The probe needs root on Linux, so the unit runs it with full privileges before each start (`ExecStartPre=-+…encryption probe --out /run/health/encryption.json`), and the unprivileged service reads the file. A Linux node's encryption state therefore refreshes at each start.
  - On Windows the service probes BitLocker itself every `HEALTH_ENCRYPTION_SECS`.
  - A missing or unreadable state is reported as unknown, never as encrypted.
- Everything shipped passes the telemetry allowlist (`health-observability`), so it carries no personal health data.

## Tests

`tests/service.rs` runs the built binary (Unix):
- it reports ready after migrating, and stops on SIGTERM with exit 0;
- it recovers after SIGKILL, and a second process cannot open its store;
- under systemd it sends `EXTEND_TIMEOUT_USEC`, `READY=1`, `WATCHDOG=1` and `STOPPING=1`;
- a UPS low-battery signal stops it gracefully;
- an old binary refuses a store that a newer one migrated (exit 4).

`tests/update.rs` installs the binary in a temporary directory and updates it:
- a package signed by another key is refused, and a signed one is staged, applied and committed once ready;
- a failed migration exits 4, and the next start rolls back and exits 6;
- a release that crashes before ready is rolled back by the previous binary's `update rollback`.

`tests/backup.rs` runs the built binary with a backup directory:
- the node takes its first backup and test restore once ready;
- `backup now` is refused while the node runs, and works once it stops;
- `backup verify`, `backup copy` to a USB directory, and `backup restore` into a fresh store that then starts and reports ready;
- a corrupt backup fails `backup test`, which records the failure for the alert, and `backup verify`.

`tests/health.rs` runs the built binary against a stand-in collector:
- `/api/admin/status` returns the report with the `backups-off` alert;
- while the collector refuses, the report says shipping is failing and the lines stay in the buffer;
- once it accepts, the buffered alert and an `indicators` line reach it.

`tests/encryption.rs` runs the built binary (Unix):
- a probe file with an unencrypted data volume reaches the status page as `disk-unencrypted`;
- an exemption shows `encryption-exempt` in its place;
- a corrupt file is `encryption-unknown`, never a pass;
- `encryption check` refuses this (unprivileged) machine with exit 10, and passes it when exempted;
- `encryption probe --out` writes the file the unit hands the service.

`tests/provision.rs` runs the built binary (Unix), with the key where systemd would unseal it (`$CREDENTIALS_DIRECTORY`):
- `provision request` refuses this unencrypted machine with exit 10, before any key or request exists;
- `provision install` refuses a chain issued to another node, a chain for another key, and a chain under another root (exit 11);
- the real chain is installed with its root, and the key is never written beside it;
- `provision status` reports what is sealed and installed, and the expiry.

Sealing with systemd-creds needs root. The edge-build systemd job provisions the installed unit end to end: it seals the key, issues the chain with `tools/pki`, installs it, and checks the journal for `identity-loaded`.
