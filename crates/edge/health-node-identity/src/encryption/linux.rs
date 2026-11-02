//! Linux: LUKS2 with a `systemd-tpm2` token sealed to PCR 7 (`systemd-cryptenroll
//! --tpm2-pcrs=7`).
//!
//! 1. `findmnt` names the device a path is mounted from.
//! 2. `lsblk --inverse` walks from it down to the disk; a `crypt` mapping over a
//!    `crypto_LUKS` device means it is encrypted, and `FSVER` gives the LUKS version.
//! 3. `cryptsetup luksDump --dump-json-metadata` (LUKS2, root) lists the tokens.
//!
//! The TPM is `/sys/class/tpm/tpm0/tpm_version_major`; Secure Boot is the `SecureBoot` EFI
//! variable.

use std::path::Path;

use serde_json::Value;

use super::{Protection, run};

const SECURE_BOOT_VAR: &str =
    "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// The LUKS device under a mounted device, from `lsblk --json --inverse --output
/// NAME,PATH,TYPE,FSTYPE,FSVER <device>`: `Ok(None)` when no `crypt` mapping is in the chain,
/// else the LUKS device's path and version.
pub fn parse_lsblk(json: &str) -> Result<Option<(String, String)>, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("lsblk output is not JSON: {e}"))?;
    let devices = value["blockdevices"]
        .as_array()
        .ok_or("lsblk output has no blockdevices")?;
    fn walk(device: &Value, under_crypt: bool) -> Option<(String, String)> {
        let text = |key: &str| device[key].as_str().unwrap_or_default().to_owned();
        if under_crypt && text("fstype") == "crypto_LUKS" {
            return Some((text("path"), text("fsver")));
        }
        let crypt = under_crypt || text("type") == "crypt";
        device["children"]
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|child| walk(child, crypt))
    }
    let found = devices.iter().find_map(|d| walk(d, false));
    if found.is_none()
        && devices
            .iter()
            .any(|d| d.to_string().contains(r#""type":"crypt""#))
    {
        return Err("a crypt mapping with no LUKS device under it".into());
    }
    Ok(found)
}

/// The protection a LUKS2 header gives, from `cryptsetup luksDump --dump-json-metadata`.
pub fn parse_luks_dump(json: &str) -> Result<Protection, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("luksDump output is not JSON: {e}"))?;
    let tokens = value["tokens"]
        .as_object()
        .ok_or("the LUKS2 header has no tokens object")?;
    let tpm: Vec<&Value> = tokens
        .values()
        .filter(|t| t["type"] == "systemd-tpm2")
        .collect();
    let sealed = tpm.iter().any(|t| {
        t["tpm2-pcrs"]
            .as_array()
            .is_some_and(|pcrs| pcrs.iter().any(|p| p.as_u64() == Some(7)))
    });
    Ok(Protection::Encrypted {
        tpm_protector: !tpm.is_empty(),
        suspended: false,
        sealed_to_secure_boot: Some(sealed),
    })
}

/// Whether Secure Boot is on, from the `SecureBoot` EFI variable: four attribute bytes, then
/// one data byte.
pub fn parse_secure_boot_var(bytes: &[u8]) -> Option<bool> {
    bytes.get(4).map(|b| *b == 1)
}

pub(super) fn tpm2() -> Option<bool> {
    let tpm = Path::new("/sys/class/tpm/tpm0");
    if !tpm.exists() {
        return Some(false);
    }
    std::fs::read_to_string(tpm.join("tpm_version_major"))
        .ok()
        .map(|v| v.trim() == "2")
}

pub(super) fn secure_boot() -> Option<bool> {
    if !Path::new("/sys/firmware/efi").exists() {
        // Booted in legacy BIOS mode: no Secure Boot.
        return Some(false);
    }
    std::fs::read(SECURE_BOOT_VAR)
        .ok()
        .and_then(|bytes| parse_secure_boot_var(&bytes))
}

pub(super) fn protection(path: &Path) -> Protection {
    match luks(path) {
        Ok(protection) => protection,
        Err(reason) => Protection::Unknown { reason },
    }
}

fn luks(path: &Path) -> Result<Protection, String> {
    let path = path.to_str().ok_or("the path is not UTF-8")?;
    let source = run(
        "findmnt",
        &["--noheadings", "--output", "SOURCE", "--target", path],
    )?;
    // A btrfs subvolume is written `/dev/sda2[/@]`.
    let device = source
        .trim()
        .split('[')
        .next()
        .unwrap_or_default()
        .to_owned();
    if !device.starts_with("/dev/") {
        return Err(format!("{path} is not on a block device ({device})"));
    }
    let tree = run(
        "lsblk",
        &[
            "--json",
            "--inverse",
            "--output",
            "NAME,PATH,TYPE,FSTYPE,FSVER",
            &device,
        ],
    )?;
    let Some((luks, version)) = parse_lsblk(&tree)? else {
        return Ok(Protection::Unencrypted);
    };
    if version != "2" {
        // LUKS1 has no tokens, so no TPM unlock.
        return Ok(Protection::Encrypted {
            tpm_protector: false,
            suspended: false,
            sealed_to_secure_boot: None,
        });
    }
    let dump = run("cryptsetup", &["luksDump", "--dump-json-metadata", &luks])
        .map_err(|e| format!("encrypted, but its LUKS2 header could not be read: {e}"))?;
    parse_luks_dump(&dump)
}
