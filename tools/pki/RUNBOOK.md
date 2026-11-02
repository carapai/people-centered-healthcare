# Root ceremony runbook (draft)

**Status: draft.** Who holds the root, the custodians and the quorum are undecided. The real ceremony is [Hold the national root ceremony and settle its custody](https://github.com/carapai/people-centered-healthcare/issues/265) (#265), which settles every point marked *custody*. The hierarchy is in ADR 0058 and [lan-security.md](../../docs/architecture/lan-security.md).

## What a ceremony does

| Ceremony | When | Steps |
|---|---|---|
| Inaugural | Once | Create the root, issue the first issuing CA and the publication-signing certificate, write the first root CRL |
| Yearly | At least every 13 months | Issue the next publication-signing certificate, then write the next root CRL |
| Issuing CA successor | In the issuing CA's fifth year | Issue its successor, then write the next root CRL |
| Compromise | When an issuing CA or the publication-signing key is suspected compromised | Revoke it, issue a replacement, then write the next root CRL |

## Roles

- **Ceremony administrator:** runs the commands and reads each one aloud before running it.
- **Witnesses:** at least two (*custody*). They check each step against this runbook and sign the record.
- **Key custodians:** hold the root key's media between ceremonies (*custody*).

## Before the ceremony

1. **The machine.** Use a laptop with no network hardware, or with Wi-Fi and Bluetooth removed. Wipe it and install a fresh OS from verified media. It never connects to a network again.
2. **The binary.** Build `pki` from a tagged commit on a connected machine (`cargo build --release -p pki`). Two people build it independently and compare SHA-256 digests. Copy it to the ceremony machine on new USB media.
3. **The media.** Prepare two new encrypted USB drives for the root key, plus one transfer drive. Each key drive gets its own passphrase, held per *custody*. The transfer drive brings in requests and takes out certificates and CRLs.
4. **The requests.**
   - Central's HSM or KMS generates the issuing CA's key and the publication-signing key, and exports only their PKCS#10 requests.
   - Put the requests on the transfer drive. Record each request's SHA-256 in the ceremony record.
5. **The names.** Agree on the root's common name, the issuing CA's common name and its DNS suffixes. The suffixes must cover every facility DNS name; adding one later needs a new issuing CA.

## Inaugural ceremony

Run every step on the ceremony machine, with the encrypted key drive mounted at `KEY` and the transfer drive at `XFER`.

1. Create the root:

   ```sh
   pki root --name "Uganda Health Root CA" --out KEY/root
   ```

   Read the root's SHA-256 aloud and record it: `sha256sum KEY/root/certificate.pem`.
2. Issue the facility issuing CA from central's request. Record the printed serial.

   ```sh
   pki issuing-ca --ca KEY/root --request XFER/issuing-ca.request.pem \
     --name "Uganda Facility Issuing CA 1" --suffixes SUFFIX[,SUFFIX] \
     --out XFER/issuing-ca.pem
   ```

3. Issue the publication-signing certificate. Record the printed serial.

   ```sh
   pki publication --ca KEY/root --request XFER/publication.request.pem \
     --name "Uganda Publication Signing 2026" --out XFER/publication.pem
   ```

4. Write the first root CRL: `pki crl --ca KEY/root --crl XFER/root.crl.pem`.
5. Copy the root certificate to the transfer drive: `cp KEY/root/certificate.pem XFER/root.pem`.
6. Copy `KEY/root` to the second key drive, and compare SHA-256 digests of every file on both drives.
7. Unmount and seal both key drives in tamper-evident bags. Record the bag numbers. Custodians store them in separate locations (*custody*).
8. Wipe the ceremony machine's disk, or seal the machine with the drives (*custody*).

## After the ceremony

- Central installs `issuing-ca.pem` beside its HSM key, and starts its own revocation list with `{"number":0,"revoked":[]}`.
- `root.pem` is published as country configuration. The enrollment installer, GPO and Firefox policy install it on browser PCs (lan-security.md).
- `root.crl.pem` enters the sync revocation lane.
- `publication.pem` goes beside the publication-signing key in central's HSM or KMS.

## Recurring ceremonies

- **Yearly:**
  1. Issue the next publication-signing certificate (step 3, with the new year in its name).
  2. Revoke the old one as superseded once the new one is in use:

     ```sh
     pki revoke --ca KEY/root --serial OLD --reason superseded --crl XFER/root.crl.pem
     ```

  3. If nothing was revoked, write the next CRL instead: `pki crl --ca KEY/root --crl XFER/root.crl.pem`.
- **Issuing CA successor:** issue the next issuing CA from a new HSM request (step 2, numbered 2). Central switches to it before the old one's fifth year ends; `pki` refuses any intermediate that would outlive its issuer. The old issuing CA stays valid for the intermediates it issued.
- **Compromise:**
  1. Revoke the compromised certificate with `--reason ca-compromise` (an issuing CA) or `key-compromise` (publication signing).
  2. Issue its replacement, and publish the CRL at once.
  3. For an issuing CA, every facility intermediate it issued is re-issued under the replacement. Tauri clients receive the new pins through sync.

## The record

The ceremony record holds:
- the date and place, and who attended in which role;
- the binary's digest and the commit it was built from;
- each request's digest, each issued serial, the root certificate's digest and each CRL number;
- the tamper-evident bag numbers;
- the witnesses' signatures.

Keep it with the custody records (*custody*).

The facility issuing CA's day-to-day work is not a ceremony. It issues intermediates through central's provisioning, and revokes them on its own list. With the tooling, that is:

```sh
pki facility --ca ISSUING --request REQUEST --node-id ID --dns NAME --out FILE
pki revoke --ca ISSUING --serial HEX --reason REASON --crl FILE
```
