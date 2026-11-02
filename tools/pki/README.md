# pki

Layer: **tooling**. The PKI's ceremonies and issuance over [`health-pki`](../../crates/infrastructure/health-pki) (ADRs 0008, 0058). The root ceremony is in [RUNBOOK.md](RUNBOOK.md).

```sh
pki root --name NAME --out DIR
pki request --name NAME --out DIR
pki issuing-ca --ca DIR --request FILE --name NAME --suffixes SUFFIX[,SUFFIX] --out FILE
pki publication --ca DIR --request FILE --name NAME --out FILE
pki facility --ca DIR --request FILE --node-id ID [--dns NAME[,NAME]] --out FILE
pki revoke --ca DIR --serial HEX --reason REASON --crl FILE
pki crl --ca DIR --crl FILE
pki verify --root FILE --chain FILE --name NAME [--crls FILE[,FILE]]
```

- **A CA directory** holds `certificate.pem`, `key.pem` and `revocations.json`. `root` creates all three.
- **`request`** writes a fresh P-256 `key.pem` and its `request.pem`. It is for test PKIs and holders without an HSM. In production the issuing CA's and publication-signing requests come from central's HSM or KMS, and the facility intermediate's request comes from the edge's keystore (#73).
- **Issuing** commands print the new certificate's serial, which is what to revoke it by.
- **`revoke`** adds the serial to the CA's `revocations.json` and writes the next CRL. **`crl`** writes the next CRL with nothing new revoked: the root's yearly reissue.
- **Reasons** for `revoke`: `unspecified`, `key-compromise`, `ca-compromise`, `superseded`, `cessation-of-operation`.
- **`verify`** checks a served chain (leaf first) against the root and CRLs, for a DNS name or IP address.
- **Files:** every output file must not exist yet, so nothing is overwritten. Keys are written owner-only (0600 on Unix).

## Tests

`tests/cli.rs` runs the binary through a ceremony. It issues the root, then an issuing CA and a facility intermediate, each from a request. It checks that a facility name outside the suffixes is refused, that keys are owner-only, and that a second root over the first is refused. A chain with the edge's leaf verifies until the intermediate is revoked; the root's yearly CRL leaves it trusted.
