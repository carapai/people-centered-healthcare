//! Windows: BitLocker with a TPM protector, read through PowerShell as LocalSystem.
//!
//! One script reads the TPM's specification version (`Win32_Tpm`), Secure Boot
//! (`Confirm-SecureBootUEFI`) and each volume's BitLocker state (`Get-BitLockerVolume`), and
//! prints them as JSON with every enum as its name.

use std::path::{Component, Path};

use health_time::Timestamp;
use serde_json::Value;

use super::{EncryptionState, Protection, Role, Volume};

const SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function Read($block) { try { & $block } catch { $null } }
$spec = Read { (Get-CimInstance -Namespace 'root/cimv2/Security/MicrosoftTpm' -ClassName Win32_Tpm).SpecVersion }
$secure = Read { Confirm-SecureBootUEFI }
$volumes = @(foreach ($mount in $env:HEALTH_PROBE_MOUNTS.Split(',')) {
  try {
    $v = Get-BitLockerVolume -MountPoint $mount
    [ordered]@{ mount = $mount; status = "$($v.VolumeStatus)"; protection = "$($v.ProtectionStatus)";
      protectors = @($v.KeyProtector | ForEach-Object { "$($_.KeyProtectorType)" }) }
  } catch { [ordered]@{ mount = $mount; error = "$_" } }
})
[ordered]@{ tpm_spec = $spec; secure_boot = $secure; volumes = $volumes } | ConvertTo-Json -Depth 4 -Compress
"#;

/// What the BitLocker probe reports: whether a TPM 2.0 is present, whether Secure Boot is on,
/// and each mount point's protection.
pub type BitLocker = (Option<bool>, Option<bool>, Vec<(String, Protection)>);

/// The TPM, Secure Boot and each mount point's protection, from the probe script's JSON.
pub fn parse_bitlocker(json: &str) -> Result<BitLocker, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|e| format!("the probe's output is not JSON: {e}"))?;
    let tpm2 = value["tpm_spec"]
        .as_str()
        .map(|spec| spec.trim_start().starts_with("2.0"));
    let tpm2 = match (tpm2, value["tpm_spec"].is_null()) {
        (Some(v), _) => Some(v),
        // No Win32_Tpm instance: no TPM.
        (None, true) => Some(false),
        (None, false) => None,
    };
    let secure_boot = value["secure_boot"].as_bool();
    // ConvertTo-Json writes a one-element array as the element itself.
    let volumes = match &value["volumes"] {
        Value::Array(items) => items.clone(),
        Value::Null => Vec::new(),
        one => vec![one.clone()],
    };
    let volumes = volumes
        .iter()
        .map(|v| {
            let mount = v["mount"].as_str().unwrap_or_default().to_owned();
            (mount, volume(v))
        })
        .collect();
    Ok((tpm2, secure_boot, volumes))
}

fn volume(v: &Value) -> Protection {
    if let Some(error) = v["error"].as_str() {
        return Protection::Unknown {
            reason: format!("Get-BitLockerVolume failed: {error}"),
        };
    }
    let text = |key: &str| v[key].as_str().unwrap_or_default();
    let protectors: Vec<&str> = match &v["protectors"] {
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        Value::String(one) => vec![one.as_str()],
        _ => Vec::new(),
    };
    match text("status") {
        "FullyEncrypted" => Protection::Encrypted {
            // TPM-only: a PIN or startup key would need a person at every boot.
            tpm_protector: protectors.contains(&"Tpm"),
            suspended: text("protection") != "On",
            sealed_to_secure_boot: None,
        },
        "FullyDecrypted" => Protection::Unencrypted,
        other => Protection::Unknown {
            reason: format!("BitLocker reports {other}"),
        },
    }
}

/// `C:` for a path on drive C.
fn mount_point(path: &Path) -> Option<String> {
    match path.components().next()? {
        Component::Prefix(prefix) => {
            let text = prefix.as_os_str().to_str()?;
            let drive = text.trim_start_matches(r"\\?\");
            (drive.len() == 2 && drive.ends_with(':')).then(|| drive.to_uppercase())
        }
        _ => None,
    }
}

pub(super) fn probe(volumes: &[(Role, &Path)], now: Timestamp) -> EncryptionState {
    let mounts: Vec<Option<String>> = volumes.iter().map(|(_, p)| mount_point(p)).collect();
    let listed: Vec<&str> = mounts.iter().flatten().map(String::as_str).collect();
    let unknown = |reason: String| EncryptionState {
        checked: now,
        tpm2: None,
        secure_boot: None,
        volumes: volumes
            .iter()
            .map(|(role, path)| Volume {
                role: *role,
                path: path.to_path_buf(),
                protection: Protection::Unknown {
                    reason: reason.clone(),
                },
            })
            .collect(),
    };
    // The drive letters go in through the environment, never spliced into the script.
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("HEALTH_PROBE_MOUNTS", listed.join(","))
        .output();
    let json = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(o) => {
            return unknown(format!(
                "the BitLocker probe failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ));
        }
        Err(e) => return unknown(format!("PowerShell could not run: {e}")),
    };
    let (tpm2, secure_boot, found) = match parse_bitlocker(&json) {
        Ok(parsed) => parsed,
        Err(reason) => return unknown(reason),
    };
    EncryptionState {
        checked: now,
        tpm2,
        secure_boot,
        volumes: volumes
            .iter()
            .zip(mounts)
            .map(|((role, path), mount)| Volume {
                role: *role,
                path: path.to_path_buf(),
                protection: match mount {
                    None => Protection::Unknown {
                        reason: "the path is not on a drive letter".into(),
                    },
                    Some(m) => found
                        .iter()
                        .find(|(mount, _)| mount.eq_ignore_ascii_case(&m))
                        .map(|(_, p)| p.clone())
                        .unwrap_or(Protection::Unknown {
                            reason: format!("BitLocker did not report {m}"),
                        }),
                },
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_points_are_drive_letters() {
        if cfg!(windows) {
            assert_eq!(
                mount_point(Path::new(r"C:\ProgramData\health")),
                Some("C:".into())
            );
            assert_eq!(mount_point(Path::new(r"\\?\d:\data")), Some("D:".into()));
        }
        assert_eq!(mount_point(Path::new("relative")), None);
    }
}
