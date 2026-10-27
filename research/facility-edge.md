# Research: running an offline Facility Edge Server on a LAN

Ticket: #3 (parent map #1). Spec: `MASTER_PROMPT.md` §§76–85 (Facility Edge, LAN transport, local auth), §93 (power interruptions).
Researched 2026-10-27, from primary sources only. "Unverified" means I could not confirm the claim from a primary source during this research.

## Summary

The closest prior art is **Open mSupply**: a Rust server (actix-web + rustls), SQLite or PostgreSQL, a Windows service built on the `windows-service` crate, mDNS/DNS-SD announcement of `_omsupply._tcp`, and a **self-signed certificate generated with `rcgen`**. Its clients are native shells (Electron and Android). They discover servers over mDNS, and they handle the self-signed certificate themselves: the device's own server is verified exactly, and any other server is trusted on first use (TOFU). Plain browsers cannot do either of those things.

The other systems surveyed do not address this problem:
- DHIS2 explicitly prefers one central server over per-site servers. Its offline story is browser caching plus the Android app.
- OpenSRP is an Android client that syncs to a central server.
- UgandaEMR/OpenMRS ships a Windows installer that bundles Tomcat and MySQL on a facility machine. Its documentation does not address LAN TLS.

The hard constraint is **browsers**. Plain `http://192.168.x.x` is not a secure context, so service workers and other secure-context APIs are unavailable. No public CA will issue a certificate that works for a private address while the site is offline. That leaves three ways to give a LAN browser a trusted HTTPS origin:
1. A **facility/national private CA**, installed on every client device.
2. A **real public domain** with DNS-01 certificates and local DNS. Under CA/B Forum SC-081, these certificates must be renewed online every ≤200 days now, ≤100 days from March 2027 and ≤47 days from March 2029.
3. **Managed browser policy** that treats the origin as secure.

Recommended baseline:
- Private CA, provisioned during device enrollment.
- Tauri clients pin the node certificate and do not depend on OS trust.
- mDNS as a convenience only, with a configurable address and QR-code pairing as the reliable path.
- `windows-service` + `sd-notify` for the OS-service layer.
- UPS plus explicit fsync durability settings.

## 1. Prior art

### Open mSupply (most relevant: Rust, LAN server, offline, sync to central)
- Architecture: "The Rust server: GraphQL API, sync, and either PostgreSQL or SQLite". "Browsers, the desktop wrapper and the Android app all talk to the same server." — https://github.com/msupply-foundation/open-msupply
- HTTP stack: `actix-web` with the `rustls` feature; `rcgen = "0.14.7"` — https://github.com/msupply-foundation/open-msupply/blob/develop/server/Cargo.toml
- TLS: `server/server/src/certs.rs` loads `certs/cert.pem` and `certs/key.pem` with rustls and uses `rcgen::generate_simple_self_signed`. A code comment reads: "in production mode certificates are required unless danger_allow_http is set in the config". — https://github.com/msupply-foundation/open-msupply/blob/develop/server/server/src/certs.rs
- Discovery: `server/server/src/discovery.rs` registers DNS-SD service `_omsupply._tcp` through `astro_dnssd`. The TXT record carries `hardware_id`, `protocol` and `client_version`. The registration is wrapped in `catch_unwind` because "Astro DNS has a unwrap in the code that seems to be None at times". — https://github.com/msupply-foundation/open-msupply/blob/develop/server/server/src/discovery.rs
- Client side:
  - The discovery page notes that "A browser cannot browse mDNS". Native hosts (Electron, Android) browse mDNS and hand the page the raw announcements.
  - The host answers the self-signed certificate error: "its own server proved exactly, anyone else's trusted on first use, and scoped to the server's origin".
  - Servers are identified "by hardware id, never address".
  - Source: https://github.com/msupply-foundation/open-msupply/blob/develop/frontend/src/discovery/README.md
- Windows service: crate `omsupply-service` depends on `windows-service = "0.8.0"` and `eventlog = "0.4.0"`. — https://github.com/msupply-foundation/open-msupply/blob/develop/server/windows/Cargo.toml
- The Docker deployment turns mDNS discovery off. — `docs/content/docker/_index.md` and `docker/compose.deploy.yaml` in the same repo.
- The mobile app has "client mode" ("connects to a server on your local network") and "server mode" ("will run a full version of the Open mSupply server locally"). — https://docs.msupply.foundation/docs/getting-started/mobile/
- Unverified: how Open mSupply handles **plain browsers** that hit the self-signed certificate (presumably a click-through warning), and whether it rotates certificates.

### DHIS2
- DHIS2 contrasts its online model with the "traditional off-line standalone style". It argues that a central server "is significantly simpler (and cheaper) than maintaining many separate servers is disparate locations".
- Its offline options are:
  - The Android app.
  - Browser-cached web data entry, with the warning that "clearing the cache of the browser will result in the lost of the data stored locally".
  - Carrying data to connectivity.
- Source: https://docs.dhis2.org/en/implement/maintenance-and-use/guidelines-for-offline-data-entry-using-dhis2.html
- The Android Capture app is offline-first and syncs to the server when it has connectivity. — https://docs.dhis2.org/en/implement/android-implementation/android-capture-overview.html
- I found no documented LAN-server mode (unverified whether one exists).

### OpenSRP
- The legacy Implementer's Guide says the app works without a connection and syncs to the central server once it reconnects. The guide is unmaintained and points to docs.opensrp.io. — https://smartregister.atlassian.net/wiki/x/AoDj
- The roadmap lists a device-to-device P2P sync library. — https://smartregister.atlassian.net/wiki/spaces/Documentation/pages/1141866497/Technical+Roadmap and https://github.com/OpenSRP/android-p2p-sync
- No LAN server.

### UgandaEMR / OpenMRS
- The installation page on the OpenMRS wiki lists Java 8, Tomcat 7 and MySQL 5.6 minimums, and points to a Windows installer from the UgandaEMR portal. It is incomplete and has TODO placeholders. — https://openmrs.atlassian.net/wiki/spaces/docs/pages/26938872/System+Setup+and+Configuration
- Unverified: whether UgandaEMR facility deployments serve LAN clients over HTTPS or plain HTTP. I found no primary documentation of LAN TLS or device enrollment.

**Takeaway:** none of the surveyed systems solves "trusted HTTPS for ordinary browsers on an offline LAN". Open mSupply sidesteps it with native shells. We should not assume browser clients will get a secure context for free.

## 2. Secure contexts: why browsers force the issue
- Potentially trustworthy origins are: `https`/`wss`/`file`, `127.0.0.0/8`, `::1`, `localhost` and `*.localhost`. Private LAN IPs are not on the list.
- "Only secure contexts are allowed to register service workers". The API check is `window.isSecureContext`.
- Source: https://developer.mozilla.org/en-US/docs/Web/Security/Secure_Contexts
- So a browser LAN client on `http://` gets no service worker, no PWA install and no other secure-context-only APIs. That matters because §77 says browser clients have no offline DB. A service worker would still be needed to cache the app shell across server restarts.
- Escape hatch for managed devices: the Chrome policy `OverrideSecurityRestrictionsOnInsecureOrigin` exempts listed origins and is "equivalent to" `--unsafely-treat-insecure-origin-as-secure`. — https://chromeenterprise.google/policies/override-security-restrictions-on-insecure-origin/ — It only works where Chrome policy can be deployed.
- Chrome **Local Network Access** permission prompt "is launching in Chrome 142". Its first milestone covers requests *from the public network to* local destinations. A page served by the edge node to its own origin is not in that scope. A centrally hosted web app calling the LAN node would be. Later milestones extend to "all cross-origin requests going to destinations on the local network". — https://developer.chrome.com/blog/local-network-access

## 3. Certificates on a LAN without internet

### Public CAs do not fit offline private addresses
- CA/B Forum Baseline Requirements maximum subscriber-certificate validity: 200 days from 2026-03-15, 100 days from 2027-03-15, 47 days from 2029-03-15. — https://cabforum.org/working-groups/server/baseline-requirements/requirements/ (§1.2.2), ballot SC-081: https://cabforum.org/2025/04/11/ballot-sc081v3-introduce-schedule-of-reducing-validity-and-data-reuse-periods/
- The ballot states that the TLS BRs cover certificates for servers "accessible through the Internet". Other uses "are not directly in scope". So private CAs are not bound by this schedule. — same ballot URL.
- Let's Encrypt IP-address certificates have been GA since 2026-01-15. They are available only on the `shortlived` profile, "valid for 160 hours, just over six days". — https://letsencrypt.org/2026/01/15/6day-and-ip-general-availability , https://letsencrypt.org/docs/profiles/
- Validation requires the CA to reach the address. Issuance for RFC 1918 addresses is therefore not possible: inferred, not explicitly stated on the pages I read. Mark as unverified. Either way, a six-day certificate is useless for a site that is offline for weeks.
- **Public-domain + DNS-01 + local DNS** is technically possible: for example, a per-facility name under a national domain resolved to the LAN IP by the facility router or the edge node. Two drawbacks:
  - Every renewal (≤200 days now, ≤47 days by 2029) needs internet.
  - LAN DNS must be under our control.
  - Feasibility at facility level, including router DNS-rebinding protection, is unverified.

### Private CA trust on client platforms
- **Apple, 398-day rule:** applies only to roots preinstalled with Apple OSes. "This change will not affect certificates issued from user-added or administrator-added Root CAs." — https://support.apple.com/en-us/102028
- **Apple, iOS 13 / macOS 10.15 rules:** "All TLS server certificates must comply":
  - RSA ≥2048.
  - SHA-2.
  - DNS names in the SAN, not the CN.
  - For certificates issued after 2019-07-01: an `id-kp-serverAuth` EKU and **validity ≤825 days**.
  - Source: https://support.apple.com/en-us/103769
  - Whether these rules also bind user-added roots is not stated explicitly beyond "All" (treat as yes, so keep leaf certificates ≤825 days).
- **iOS manual root install:** the user must also enable it under Settings > General > About > Certificate Trust Settings. Profiles deployed by MDM or Apple Configurator are trusted automatically. — https://support.apple.com/en-us/102390
- **Chrome (Windows/macOS):** Chrome's verifier consumes the Windows Local Machine and Current User stores and the macOS keychains.
  - CT enforcement and the 398-day rule are listed as rules for certificates chaining to the Chrome Root Store, implying they do not apply to locally trusted roots (inference).
  - On iOS, Chrome uses Apple's verifier.
  - Android/Linux handling is not described on the page (unverified).
  - Source: https://chromium.googlesource.com/chromium/src/+/main/net/data/ssl/chrome_root_store/faq.md
- **Firefox:** "does not use the operating system's certificate store by default". The `security.enterprise_roots.enabled` preference covers Windows (since 49) and macOS (since 63). The enterprise policy `ImportEnterpriseRoots` / `Certificates.Install` is the recommended route. On Linux, roots are added through p11-kit or `certutil`. — https://wiki.mozilla.org/CA/AddRootToFirefox
- **Android apps:** apps targeting API 24+ trust only system CAs by default. They must opt in to user CAs via `network_security_config`. — https://developer.android.com/privacy-and-security/security-config
- Whether **Chrome on Android** trusts user-installed CAs is unverified: I found no primary source. Firefox on Android is also unverified.

### Generating and serving certificates in Rust
- `rcgen` 0.14.x supports CA certificates (`IsCa`), signing (`signed_by`) and `SanType::IpAddress(IpAddr)` and `DnsName` SANs. — https://docs.rs/rcgen/latest/rcgen/ , https://docs.rs/rcgen/latest/rcgen/enum.SanType.html
- That covers a node-local or national-intermediate CA issuing leaf certificates with both IP and hostname SANs.
- rustls serves these (as in Open mSupply above).

## 4. OS service layer (Rust)
- **Windows:** `windows-service` 0.8.1 (Mullvad, MIT/Apache-2.0). It provides `define_windows_service!`, a control handler for stop/interrogate and other events, and `set_service_status` with wait hints for long pending states. It includes a `service_manager` module. — https://docs.rs/windows-service/latest/windows_service/ — Open mSupply uses it in production (above).
- **Windows restart on failure:** use SCM failure actions (`SERVICE_FAILURE_ACTIONS`).
  - A service "is considered failed when it terminates without reporting a status of SERVICE_STOPPED".
  - The action array is indexed by failure count, and the count resets after `dwResetPeriod`.
  - Source: https://learn.microsoft.com/en-us/windows/win32/api/winsvc/ns-winsvc-service_failure_actionsw
- **Linux:** a systemd unit with `Type=notify`. The service sends `READY=1` when startup is finished. `WatchdogSec=` requires periodic `WATCHDOG=1`; otherwise systemd kills the service with SIGABRT. `Restart=on-failure`/`always`. Stop sends `KillSignal=` (SIGTERM by default), then SIGKILL after `TimeoutStopSec=`. — https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html
- `STOPPING=1` and `EXTEND_TIMEOUT_USEC=` let a long DB shutdown or migration avoid a SIGKILL. — https://www.freedesktop.org/software/systemd/man/latest/sd_notify.html
- Crate `sd-notify` 0.5.0 (pure Rust) provides `notify`, `watchdog_enabled` and `listen_fds`. — https://docs.rs/sd-notify/latest/sd_notify/

## 5. LAN discovery (mDNS / DNS-SD)
- **Crates:**
  - `mdns-sd` 0.21.x supports `register` and `browse`. Its dependency list is pure Rust, with no Bonjour/Avahi bindings (inferred from dependencies). — https://docs.rs/mdns-sd/latest/mdns_sd/
  - `astro-dnssd` 0.3.7 wraps system libraries: the Bonjour SDK on Windows (optional since 0.3 via the `win-bonjour` feature) and `avahi-compat-libdns_sd` on Linux. — https://github.com/astrohq/astro-dnssd — Open mSupply has had to catch panics from it (above).
- **Browsers cannot browse DNS-SD.** At most they can resolve a `.local` hostname if the OS can. Android's resolver has supported `.local` mDNS resolution since November 2021, excluding VPN and mobile-data connections. — https://source.android.com/docs/core/ota/modular-system/dns-resolver
- Native Android apps use NSD (`NsdManager`). — https://developer.android.com/training/connect-devices-wirelessly/nsd
- Windows `.local` resolution in browsers is unverified.
- **Wi-Fi reliability (RFC 9119):**
  - Multicast is sent "at the slowest rate of all the connected devices".
  - It gets no ACKs, and loss of "5% or more" is not uncommon.
  - It is delayed by power-saving stations.
  - mDNS "is often dropped by operators".
  - Multicast-to-unicast conversion is a mitigation.
  - Source: https://www.rfc-editor.org/rfc/rfc9119.html
- AP client isolation is not covered by RFC 9119. That it blocks mDNS and client-to-server traffic on consumer APs is general knowledge but unverified here.

## 6. Device enrollment and revocation
- Primary-source material is limited to Open mSupply's model: identity by hardware ID, the own server pinned exactly, remote servers TOFU, scoped per origin (see §1).
- I found no comparable system documenting per-device revocation on a LAN. Design is open: see options below.

## 7. Power loss / UPS
- **SurrealDB durability:** `SURREAL_DATASTORE_SYNC_DATA` (since v3.0.0), default `every`, values `never|every|duration`. It applies to the memory, RocksDB and SurrealKV engines. — https://surrealdb.com/docs/surrealdb/cli/env
- The edge node must keep `every` (or set an explicit, small duration) and must never set `never`.
- SurrealDB's blog says SurrealKV 1.0 is replacing RocksDB. — https://surrealdb.com/blog/we-are-replacing-rocksdb-with-surrealkv-1-0 — The timeline for SurrealDB 4 is unverified.
- **UPS signalling:** Network UPS Tools (GPL) monitors UPSs and PDUs. Its Windows branch was merged in 2022, but per the project MSI packaging and some drivers still need community work. — https://networkupstools.org/
- The upsmon `SHUTDOWNCMD` mechanism was not confirmed from the page I fetched (unverified details).
- Graceful-stop hooks: the Windows SCM stop control (via `windows-service`) and systemd SIGTERM / `STOPPING=1` (above).

## Implications for the decision (feeds #18, #19)

### Options for browser HTTPS on the LAN
| Option | Works offline indefinitely | Per-device setup | Main risk |
|---|---|---|---|
| A. Private CA (national root → facility/node intermediate, issued with `rcgen`), root installed at enrollment | Yes (we choose lifetimes; keep leaf ≤825 days for Apple) | Install root on every browser device. iOS needs a manual trust toggle unless MDM. Firefox needs a policy or pref. | Root-key compromise affects every enrolled device. Install UX on unmanaged phones. |
| B. Public domain per facility + DNS-01 + local DNS | No: renewal online every ≤200 days now, ≤47 days by 2029 | None on clients | A facility offline longer than the certificate lifetime loses HTTPS. Depends on LAN DNS control. Unverified at facility scale. |
| C. Plain HTTP + Chrome `OverrideSecurityRestrictionsOnInsecureOrigin` | Yes | A Chrome policy on each device (needs management) | No transport encryption on Wi-Fi. Violates §82 "Authenticated HTTPS". Not acceptable for clinical data. |
| D. Self-signed + click-through (TOFU in the browser) | Yes | Users accept a warning | Trains users to ignore TLS warnings. A secure context is still granted after bypass in Chrome (unverified). Poor UX. |
| E. Native/Tauri clients only for LAN; browsers optional | Yes | App install | Tauri can pin the node certificate itself (Open mSupply pattern), so no OS trust is needed. |

**Leaning:** A + E. Tauri and native clients pin the node certificate and identity, so they need no OS trust. Browser clients go through enrollment that installs the private root, done via MDM, Windows GPO or the Firefox policy where devices are managed. Keep B as an optional extra for connected facilities. Reject C and D for production.

### Discovery
- Use mDNS/DNS-SD (`mdns-sd`, pure Rust; this avoids `astro-dnssd`'s system-library and panic issues) only as a hint inside Tauri/native clients.
- Make the **configured address plus QR-code pairing** (node URL, node ID, certificate fingerprint) the primary path.
- Identify the node by a stable node ID or key fingerprint, never by IP (Open mSupply lesson).

### Service layer
- Use `windows-service` with SCM failure actions configured by the installer.
- On Linux, use a systemd `Type=notify` unit with `WatchdogSec`, `Restart=on-failure`, a generous `TimeoutStopSec`, and `sd-notify` for `READY`/`WATCHDOG`/`STOPPING`/`EXTEND_TIMEOUT_USEC`.
- Use one binary with an `--service` mode, plus a dev foreground mode.

### Risks
- iOS/Android unmanaged phones may be impractical as browser clients (manual root trust). This may force "browser clients = managed PCs + Tauri".
- Private CA key custody and rotation need a governance decision. That belongs to the security ticket.
- mDNS will fail on some APs (multicast filtering or isolation), so the fallback is mandatory, not optional.
- SurrealDB storage-engine transition (RocksDB → SurrealKV) means durability settings must be re-validated on upgrade.

### Open questions
1. Is a national private root acceptable to the MoH, or do we need a per-facility root with no national hierarchy?
2. Will facility browser clients be managed (GPO/MDM)? This decides how practical option A is.
3. Leaf certificate lifetime and rotation cadence offline. ≤825 days is the Apple ceiling, but shorter needs an online or enrollment touchpoint.
4. Revocation: is it enforced by the node (device-key allowlist, mTLS or a signed device token) rather than by CRL/OCSP, which browsers will not check offline for private CAs (unverified)?
5. Does Chrome on Android honour user-installed roots (unverified)? If not, Android browser clients are out, and Android must use the Tauri/native app.
6. Which UPS integration do we support: NUT on Linux and vendor or NUT on Windows? Is a "power-loss" signal exposed to the node for a proactive graceful stop?
