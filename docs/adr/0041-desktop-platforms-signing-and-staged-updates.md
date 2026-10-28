# v1 desktops target Windows and Ubuntu LTS, are signed through cloud HSMs, and update in stages from central

**Tauri version.** At least 2.11.6, because of the 2026 origin and channel advisories. Pinned to the 2.12 line.

**Targets:**

| Platform | Packages | Engine |
|---|---|---|
| Windows 10/11 x64 | Per-machine NSIS installer, in a standard-bootstrapper variant and a WebView2 `offlineInstaller` variant for offline provisioning | WebView2 |
| Ubuntu 22.04 and 24.04 LTS x64 | AppImage and deb | WebKitGTK 4.1 |

The React build targets es2022, and end-to-end tests run on both engines. **macOS is deferred.**

**Signing:**

| Artifact | Signing |
|---|---|
| Windows installer | Authenticode via Azure Artifact Signing |
| Tauri updater artifacts | Updater key in central's KMS/HSM, with an offline backup from a key ceremony |
| AppImage | GPG |
| deb | Signed apt repository |

**Updates:**
- A dynamic updater endpoint at central returns the version allowed for each facility, so roll-outs are staged like package activations.
- Offline sites install a signed installer from USB.
- An update installs at the next restart, never mid-session. A local backup is taken first, and forward-only migrations run before the app reports ready.
- A desktop must stay within N-1 of its upstream.

**Peripherals.** Printing uses the OS. Barcode scanners are supported in keyboard-wedge mode. Analyser links are out of v1.

We chose this because facility PCs are overwhelmingly Windows, with some Ubuntu. The Tauri updater requires its own signature, and losing that key would strand every installed client. Unsigned Windows apps are also blocked by Smart App Control.

## Considered Options

- **macOS in v1.** Deferred: it needs an Apple developer account and Mac runners, for hardware facilities rarely use.
- **An OV certificate on a build machine.** Rejected: there is a key-theft risk, and EV no longer buys SmartScreen reputation.
- **Auto-installing updates immediately.** Rejected: it would interrupt clinical sessions and could run migrations under load.

## Consequences

- deb users update through the apt repository rather than the Tauri updater.
- Custody of the signing keys joins the PKI governance fog.
