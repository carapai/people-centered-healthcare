# The breached-password filter

`breached-passwords.bloom` is the breached-password list that every new password is checked against (offline-auth.md § Credentials; #189). It is a Bloom filter (format in `src/breached.rs`) built into the binary, so central and edges check the same way, offline.

| | |
|---|---|
| Source | [SecLists](https://github.com/danielmiessler/SecLists) `Passwords/Common-Credentials/xato-net-10-million-passwords-1000000.txt` at commit `00f80ec9cc39bed63ee40cf6805a6b1b77e87644` |
| Source SHA-256 | `424a3e03a17df0a2bc2b3ca749d81b04e79d59cb7aeec8876a5a3f308d0caf51` |
| Licence | MIT, copyright Daniel Miessler (`SECLISTS-LICENSE`) |
| Entries | 999,996 distinct passwords |
| False positives | 0.1% (k = 10), 1.8 MB |
| Filter SHA-256 | `d21354ccd02f9af0f21f02aadc5065755108db2795357876f680c5c5c96efc3b` |

To rebuild it, for example from a newer commit:

```sh
cargo run -p health-auth --example breached_filter -- xato-net-10-million-passwords-1000000.txt crates/infrastructure/health-auth/data/breached-passwords.bloom
```

Then update this table.
