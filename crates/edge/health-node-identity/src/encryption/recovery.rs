//! Disk recovery keys (ADR 0048 § Recovery keys): what unlocks an edge's encrypted volume when
//! the TPM cannot, generated at provisioning and escrowed at central, never kept on site.
//!
//! The edge generates each key itself, as a BitLocker **recovery password**: 48 digits in 8
//! groups of 6, each group a multiple of 11 below 720,896. Both platforms take that format, and
//! it reads well over the phone. [`RecoveryProtectors`] adds a key to a volume and removes one,
//! by an id that is not secret:
//! - **Windows:** `Add-BitLockerKeyProtector -RecoveryPasswordProtector`. The id is the new
//!   protector's `KeyProtectorId`. The key and the mount point reach PowerShell through the
//!   environment, never the command line.
//! - **Linux:** `cryptsetup luksAddKey`, with the new key in a 0600 file that is removed at
//!   once. cryptsetup prompts on the terminal for a secret the volume already has: the install
//!   passphrase, or, at a rotation, the key just read out. The id is the new key slot, found by
//!   comparing the header before and after. A key is removed with `cryptsetup luksKillSlot`.
//!
//! The commands need administrator rights and an encrypted volume. They are checked on facility
//! hardware; [`parse_keyslots`] and [`parse_protector_ids`] are tested here.

use std::path::Path;

use serde_json::Value;

/// A recovery password from `draw`, one random 16-bit value per group.
pub fn recovery_password(mut draw: impl FnMut() -> u16) -> String {
    (0..8)
        .map(|_| format!("{:06}", u32::from(draw()) * 11))
        .collect::<Vec<_>>()
        .join("-")
}

/// A recovery password from the operating system's random source.
pub fn new_recovery_password() -> String {
    use rand_core::RngCore as _;
    recovery_password(|| {
        let mut bytes = [0u8; 2];
        rand_core::OsRng.fill_bytes(&mut bytes);
        u16::from_le_bytes(bytes)
    })
}

/// Whether `text` is a recovery password BitLocker accepts.
pub fn valid_recovery_password(text: &str) -> bool {
    let groups: Vec<&str> = text.split('-').collect();
    groups.len() == 8
        && groups.iter().all(|group| {
            group.len() == 6
                && group.bytes().all(|b| b.is_ascii_digit())
                && group
                    .parse::<u32>()
                    .is_ok_and(|n| n % 11 == 0 && n < 720_896)
        })
}

/// The key slots in a LUKS2 header, from `cryptsetup luksDump --dump-json-metadata`.
pub fn parse_keyslots(json: &str) -> Result<Vec<String>, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("luksDump output is not JSON: {e}"))?;
    let mut slots: Vec<String> = value["keyslots"]
        .as_object()
        .ok_or("the LUKS2 header has no keyslots object")?
        .keys()
        .cloned()
        .collect();
    slots.sort_by_key(|slot| slot.parse::<u32>().unwrap_or(u32::MAX));
    Ok(slots)
}

/// The protector ids the add script printed: a JSON array, or one string.
pub fn parse_protector_ids(json: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(json.trim())
        .map_err(|e| format!("the BitLocker script's output is not JSON: {e}"))?;
    match value {
        Value::String(id) => Ok(vec![id]),
        Value::Array(ids) => ids
            .into_iter()
            .map(|id| match id {
                Value::String(id) => Ok(id),
                other => Err(format!("a protector id that is not a string: {other}")),
            })
            .collect(),
        Value::Null => Ok(Vec::new()),
        other => Err(format!("unexpected protector ids: {other}")),
    }
}

/// The one id in `ids`, or why there is not exactly one.
pub fn the_new_one(ids: Vec<String>) -> Result<String, String> {
    match <[String; 1]>::try_from(ids) {
        Ok([id]) => Ok(id),
        Err(ids) => Err(format!(
            "expected one new recovery key, found {}",
            ids.len()
        )),
    }
}

/// Adding and removing recovery keys on the host's encrypted volumes.
pub trait RecoveryProtectors {
    /// The encrypted volume holding `path`, as the platform names it (a LUKS device, or a
    /// drive letter); `None` if it is not encrypted.
    fn volume_of(&self, path: &Path) -> Result<Option<String>, String>;
    /// Adds `password` to `volume` as a recovery key; returns its id.
    fn add(&self, volume: &str, password: &str) -> Result<String, String>;
    /// Removes the recovery key `id` from `volume`.
    fn remove(&self, volume: &str, id: &str) -> Result<(), String>;
}

/// The host's own tools.
pub struct SystemProtectors;

const ADD_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Ids { @((Get-BitLockerVolume -MountPoint $env:HEALTH_RECOVERY_MOUNT).KeyProtector |
  Where-Object { "$($_.KeyProtectorType)" -eq 'RecoveryPassword' } | ForEach-Object { "$($_.KeyProtectorId)" }) }
$before = Ids
Add-BitLockerKeyProtector -MountPoint $env:HEALTH_RECOVERY_MOUNT -RecoveryPasswordProtector -RecoveryPassword $env:HEALTH_RECOVERY_PASSWORD | Out-Null
ConvertTo-Json -Compress -InputObject @(Ids | Where-Object { $before -notcontains $_ })
"#;

const REMOVE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Remove-BitLockerKeyProtector -MountPoint $env:HEALTH_RECOVERY_MOUNT -KeyProtectorId $env:HEALTH_RECOVERY_ID | Out-Null
"#;

fn powershell(script: &str, env: &[(&str, &str)]) -> Result<String, String> {
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .envs(env.iter().copied())
        .output()
        .map_err(|e| format!("PowerShell could not run: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "BitLocker refused: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn keyslots(device: &str) -> Result<Vec<String>, String> {
    parse_keyslots(&super::run(
        "cryptsetup",
        &["luksDump", "--dump-json-metadata", device],
    )?)
}

/// Writes `secret` to a new file only the owner can read, under `/run` where it is in memory.
#[cfg(unix)]
fn secret_file(secret: &str) -> Result<std::path::PathBuf, String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let dir = if Path::new("/run").is_dir() {
        std::path::PathBuf::from("/run")
    } else {
        std::env::temp_dir()
    };
    let path = dir.join(format!("health-recovery-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("cannot write the new key for cryptsetup: {e}"))?;
    file.write_all(secret.as_bytes())
        .map_err(|e| format!("cannot write the new key for cryptsetup: {e}"))?;
    Ok(path)
}

#[cfg(not(unix))]
fn secret_file(_: &str) -> Result<std::path::PathBuf, String> {
    Err("LUKS is Linux only".into())
}

impl RecoveryProtectors for SystemProtectors {
    fn volume_of(&self, path: &Path) -> Result<Option<String>, String> {
        if cfg!(windows) {
            Ok(super::windows::mount_point(path))
        } else if cfg!(target_os = "linux") {
            Ok(super::linux::luks_device(path)?.map(|(device, _)| device))
        } else {
            Err("recovery keys are managed on Linux and Windows only".into())
        }
    }

    fn add(&self, volume: &str, password: &str) -> Result<String, String> {
        if cfg!(windows) {
            return the_new_one(parse_protector_ids(&powershell(
                ADD_SCRIPT,
                &[
                    ("HEALTH_RECOVERY_MOUNT", volume),
                    ("HEALTH_RECOVERY_PASSWORD", password),
                ],
            )?)?);
        }
        let before = keyslots(volume)?;
        let file = secret_file(password)?;
        // cryptsetup asks on the terminal for a secret the volume already has.
        let status = std::process::Command::new("cryptsetup")
            .args(["luksAddKey", volume])
            .arg(&file)
            .status();
        let _ = std::fs::remove_file(&file);
        match status {
            Ok(status) if status.success() => {}
            Ok(status) => return Err(format!("cryptsetup luksAddKey failed ({status})")),
            Err(e) => return Err(format!("cryptsetup could not run: {e}")),
        }
        let after = keyslots(volume)?;
        the_new_one(after.into_iter().filter(|s| !before.contains(s)).collect())
    }

    fn remove(&self, volume: &str, id: &str) -> Result<(), String> {
        if cfg!(windows) {
            return powershell(
                REMOVE_SCRIPT,
                &[
                    ("HEALTH_RECOVERY_MOUNT", volume),
                    ("HEALTH_RECOVERY_ID", id),
                ],
            )
            .map(|_| ());
        }
        // `-q`: the slot is named, so cryptsetup needs no other passphrase to confirm.
        super::run("cryptsetup", &["luksKillSlot", "-q", volume, id]).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_passwords_are_what_bitlocker_accepts() {
        let mut n = 0u16;
        let password = recovery_password(|| {
            n = n.wrapping_add(9000);
            n
        });
        // The eighth draw wraps: 72,000 - 65,536 = 6,464, times 11.
        assert_eq!(
            password,
            "099000-198000-297000-396000-495000-594000-693000-071104"
        );
        assert!(valid_recovery_password(&password));
        assert!(valid_recovery_password(&recovery_password(|| u16::MAX)));
        let fresh = new_recovery_password();
        assert!(valid_recovery_password(&fresh), "{fresh}");
        assert_ne!(fresh, new_recovery_password());
        for wrong in [
            "099000-198000-297000-396000-495000-594000-693000",
            "099001-198000-297000-396000-495000-594000-693000-071896",
            "720896-198000-297000-396000-495000-594000-693000-071896",
            "99000-198000-297000-396000-495000-594000-693000-0718960",
            "",
        ] {
            assert!(!valid_recovery_password(wrong), "{wrong}");
        }
    }

    #[test]
    fn key_slots_and_protector_ids_read_from_the_tools() {
        let dump = r#"{"keyslots":{"10":{"type":"luks2"},"0":{"type":"luks2"},"2":{"type":"luks2"}},"tokens":{}}"#;
        assert_eq!(
            parse_keyslots(dump),
            Ok(vec!["0".into(), "2".into(), "10".into()])
        );
        assert!(parse_keyslots("{}").is_err());
        assert_eq!(
            parse_protector_ids(r#"["{A1}"]"#),
            Ok(vec!["{A1}".to_owned()])
        );
        assert_eq!(
            parse_protector_ids(r#""{A1}""#),
            Ok(vec!["{A1}".to_owned()])
        );
        assert_eq!(parse_protector_ids("[]"), Ok(Vec::new()));
        assert!(parse_protector_ids("[1]").is_err());
        assert_eq!(the_new_one(vec!["3".into()]), Ok("3".into()));
        assert!(the_new_one(Vec::new()).is_err());
        assert!(the_new_one(vec!["3".into(), "4".into()]).is_err());
    }
}
