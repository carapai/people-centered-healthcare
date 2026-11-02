//! `pki` runs the PKI's ceremonies and issuance over `health-pki`. See README.md and RUNBOOK.md.
//!
//! A CA directory holds `certificate.pem`, `key.pem` and `revocations.json`. Every file is
//! created new and never overwritten, and keys are written owner-only.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use health_pki::{Authority, Facility, IssuingCa, Reason, Revocations, Trust, new_key, request};
use health_time::Timestamp;

const USAGE: &str = "usage:
  pki root --name NAME --out DIR
  pki request --name NAME --out DIR
  pki issuing-ca --ca DIR --request FILE --name NAME --suffixes SUFFIX[,SUFFIX] --out FILE
  pki publication --ca DIR --request FILE --name NAME --out FILE
  pki facility --ca DIR --request FILE --node-id ID [--dns NAME[,NAME]] --out FILE
  pki revoke --ca DIR --serial HEX --reason REASON --crl FILE
  pki crl --ca DIR --crl FILE
  pki verify --root FILE --chain FILE --name NAME [--crls FILE[,FILE]]
reasons: unspecified, key-compromise, ca-compromise, superseded, cessation-of-operation";

const CERTIFICATE: &str = "certificate.pem";
const KEY: &str = "key.pem";
const REVOCATIONS: &str = "revocations.json";

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let mut options = HashMap::new();
    while let Some(flag) = args.next() {
        let Some(name) = flag.strip_prefix("--") else {
            bail!("unexpected {flag}\n{USAGE}")
        };
        let value = args
            .next()
            .with_context(|| format!("--{name} needs a value\n{USAGE}"))?;
        options.insert(name.to_owned(), value);
    }
    let get = |name: &str| -> Result<&str> {
        options
            .get(name)
            .map(String::as_str)
            .with_context(|| format!("--{name} is required\n{USAGE}"))
    };
    let path = |name: &str| get(name).map(PathBuf::from);
    let list = |name: &str| -> Vec<String> {
        options
            .get(name)
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default()
    };
    let now = Timestamp::now();
    match command.as_str() {
        "root" => {
            let out = path("out")?;
            let root = Authority::root(get("name")?, now)?;
            fs::create_dir_all(&out)?;
            write_key(&out.join(KEY), &root.key_pem())?;
            write_new(&out.join(CERTIFICATE), &root.certificate_pem())?;
            write_new(
                &out.join(REVOCATIONS),
                &serde_json::to_string_pretty(&Revocations::default())?,
            )?;
            eprintln!("root written to {}", out.display());
        }
        "request" => {
            let out = path("out")?;
            let key = new_key()?;
            fs::create_dir_all(&out)?;
            write_key(&out.join(KEY), &key.serialize_pem())?;
            write_new(&out.join("request.pem"), &request(&key, get("name")?)?)?;
            eprintln!("key and request written to {}", out.display());
        }
        "issuing-ca" => {
            let ca = IssuingCa {
                common_name: get("name")?.to_owned(),
                dns_suffixes: list("suffixes"),
            };
            let issued =
                load(&path("ca")?)?.issue_issuing_ca(&read(&path("request")?)?, &ca, now)?;
            write_new(&path("out")?, &issued.pem())?;
            eprintln!("issued serial {}", issued.serial);
        }
        "publication" => {
            let issued = load(&path("ca")?)?.issue_publication(
                &read(&path("request")?)?,
                get("name")?,
                now,
            )?;
            write_new(&path("out")?, &issued.pem())?;
            eprintln!("issued serial {}", issued.serial);
        }
        "facility" => {
            let facility = Facility {
                node_id: get("node-id")?.to_owned(),
                dns_names: list("dns"),
            };
            let issued =
                load(&path("ca")?)?.issue_facility(&read(&path("request")?)?, &facility, now)?;
            write_new(&path("out")?, &issued.pem())?;
            eprintln!("issued serial {}", issued.serial);
        }
        "revoke" | "crl" => {
            let dir = path("ca")?;
            let ca = load(&dir)?;
            let file = dir.join(REVOCATIONS);
            let mut revocations: Revocations = serde_json::from_str(&read(&file)?)
                .with_context(|| format!("reading {}", file.display()))?;
            if command == "revoke" {
                let serial = get("serial")?.parse()?;
                let reason = serde_json::from_value::<Reason>(get("reason")?.into())
                    .with_context(|| format!("--reason\n{USAGE}"))?;
                revocations.revoke(serial, reason, now);
            }
            let crl = revocations.crl(&ca, now)?;
            write_new(&path("crl")?, &crl)?;
            replace(&file, &serde_json::to_string_pretty(&revocations)?)?;
            eprintln!(
                "CRL {} lists {} revoked",
                revocations.number,
                revocations.revoked.len()
            );
        }
        "verify" => {
            let mut trust = Trust::new(&read(&path("root")?)?)?;
            for crl in list("crls") {
                trust.add_crl(&read(Path::new(&crl))?)?;
            }
            trust.verify_server(&read(&path("chain")?)?, get("name")?, now)?;
            eprintln!("trusted for {}", get("name")?);
        }
        _ => bail!("{USAGE}"),
    }
    Ok(())
}

fn load(dir: &Path) -> Result<Authority> {
    Ok(Authority::from_pem(
        &read(&dir.join(CERTIFICATE))?,
        &read(&dir.join(KEY))?,
    )?)
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

/// Writes a file that must not exist yet.
fn write_new(path: &Path, contents: &str) -> Result<()> {
    create(path, false)?.write_all(contents.as_bytes())?;
    Ok(())
}

/// Writes a private key that must not exist yet, readable by its owner only.
fn write_key(path: &Path, contents: &str) -> Result<()> {
    create(path, true)?.write_all(contents.as_bytes())?;
    Ok(())
}

fn create(path: &Path, private: bool) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    options
        .open(path)
        .with_context(|| format!("creating {} (it must not exist yet)", path.display()))
}

/// Replaces `path` through a temporary file, so a failure leaves the old list intact.
fn replace(path: &Path, contents: &str) -> Result<()> {
    let temporary = path.with_extension("json.new");
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
