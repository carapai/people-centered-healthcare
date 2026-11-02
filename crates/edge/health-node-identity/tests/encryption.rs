//! The full-disk encryption check (ADR 0048) against the platforms' real output shapes, and the
//! provisioning gate it feeds.

use std::path::PathBuf;

use health_node_identity::encryption::{
    EncryptionState, Exemption, Finding, Gate, Protection, Role, Volume, gate, parse_bitlocker,
    parse_lsblk, parse_luks_dump, parse_secure_boot_var, probe, system_root,
};
use health_time::Timestamp;

fn at() -> Timestamp {
    "2026-11-02T09:00:00Z".parse().expect("time")
}

/// `lsblk --json --inverse` for Ubuntu's default encrypted install: ext4 on LVM on LUKS2.
const LSBLK_LUKS_LVM: &str = r#"{"blockdevices":[{"name":"ubuntu--vg-ubuntu--lv","path":"/dev/mapper/ubuntu--vg-ubuntu--lv","type":"lvm","fstype":"ext4","fsver":"1.0","children":[{"name":"dm_crypt-0","path":"/dev/mapper/dm_crypt-0","type":"crypt","fstype":"LVM2_member","fsver":"LVM2 001","children":[{"name":"nvme0n1p3","path":"/dev/nvme0n1p3","type":"part","fstype":"crypto_LUKS","fsver":"2","children":[{"name":"nvme0n1","path":"/dev/nvme0n1","type":"disk","fstype":null,"fsver":null}]}]}]}]}"#;
const LSBLK_PLAIN: &str = r#"{"blockdevices":[{"name":"sda1","path":"/dev/sda1","type":"part","fstype":"ext4","fsver":"1.0","children":[{"name":"sda","path":"/dev/sda","type":"disk","fstype":null,"fsver":null}]}]}"#;
const LSBLK_LUKS1: &str = r#"{"blockdevices":[{"name":"root","path":"/dev/mapper/root","type":"crypt","fstype":"ext4","fsver":"1.0","children":[{"name":"sda2","path":"/dev/sda2","type":"part","fstype":"crypto_LUKS","fsver":"1"}]}]}"#;

/// `cryptsetup luksDump --dump-json-metadata` after `systemd-cryptenroll --tpm2-device=auto
/// --tpm2-pcrs=7`, trimmed to what the check reads.
const LUKS_TPM2_PCR7: &str = r#"{"keyslots":{"0":{"type":"luks2"},"1":{"type":"luks2"}},"tokens":{"0":{"type":"systemd-tpm2","keyslots":["1"],"tpm2-pcrs":[7],"tpm2-bank":"sha256"}},"segments":{},"digests":{},"config":{}}"#;
const LUKS_TPM2_NO_PCRS: &str = r#"{"keyslots":{"0":{"type":"luks2"},"1":{"type":"luks2"}},"tokens":{"0":{"type":"systemd-tpm2","keyslots":["1"],"tpm2-pcrs":[]}},"segments":{},"digests":{},"config":{}}"#;
const LUKS_PASSPHRASE: &str =
    r#"{"keyslots":{"0":{"type":"luks2"}},"tokens":{},"segments":{},"digests":{},"config":{}}"#;

fn compliant() -> EncryptionState {
    let encrypted = Protection::Encrypted {
        tpm_protector: true,
        suspended: false,
        sealed_to_secure_boot: Some(true),
    };
    EncryptionState {
        checked: at(),
        tpm2: Some(true),
        secure_boot: Some(true),
        volumes: vec![
            Volume {
                role: Role::System,
                path: "/".into(),
                protection: encrypted.clone(),
            },
            Volume {
                role: Role::Data,
                path: "/var/lib/health/data".into(),
                protection: encrypted,
            },
        ],
    }
}

#[test]
fn lsblk_finds_luks_under_lvm_and_nothing_on_plain_disks() {
    assert_eq!(
        parse_lsblk(LSBLK_LUKS_LVM).expect("parse"),
        Some(("/dev/nvme0n1p3".into(), "2".into()))
    );
    assert_eq!(parse_lsblk(LSBLK_PLAIN).expect("parse"), None);
    assert_eq!(
        parse_lsblk(LSBLK_LUKS1).expect("parse"),
        Some(("/dev/sda2".into(), "1".into()))
    );
    assert!(parse_lsblk("not json").is_err());
    // A crypt mapping that is not LUKS (plain dm-crypt) cannot hold a TPM token.
    let plain_crypt = r#"{"blockdevices":[{"name":"c","path":"/dev/mapper/c","type":"crypt","fstype":"ext4","children":[{"name":"sda2","path":"/dev/sda2","type":"part","fstype":null}]}]}"#;
    assert!(parse_lsblk(plain_crypt).is_err());
}

#[test]
fn luks_headers_show_the_tpm_token_and_its_pcrs() {
    assert_eq!(
        parse_luks_dump(LUKS_TPM2_PCR7).expect("parse"),
        Protection::Encrypted {
            tpm_protector: true,
            suspended: false,
            sealed_to_secure_boot: Some(true)
        }
    );
    assert_eq!(
        parse_luks_dump(LUKS_TPM2_NO_PCRS).expect("parse"),
        Protection::Encrypted {
            tpm_protector: true,
            suspended: false,
            sealed_to_secure_boot: Some(false)
        }
    );
    assert_eq!(
        parse_luks_dump(LUKS_PASSPHRASE).expect("parse"),
        Protection::Encrypted {
            tpm_protector: false,
            suspended: false,
            sealed_to_secure_boot: Some(false)
        }
    );
}

#[test]
fn secure_boot_is_the_variables_data_byte() {
    assert_eq!(parse_secure_boot_var(&[6, 0, 0, 0, 1]), Some(true));
    assert_eq!(parse_secure_boot_var(&[6, 0, 0, 0, 0]), Some(false));
    assert_eq!(parse_secure_boot_var(&[6, 0, 0]), None);
}

#[test]
fn bitlocker_states_map_to_protection() {
    let json = r#"{"tpm_spec":"2.0, 0, 1.59","secure_boot":true,"volumes":[
        {"mount":"C:","status":"FullyEncrypted","protection":"On","protectors":["Tpm","RecoveryPassword"]},
        {"mount":"D:","status":"FullyEncrypted","protection":"Off","protectors":["Tpm"]},
        {"mount":"E:","status":"FullyEncrypted","protection":"On","protectors":["TpmPin"]},
        {"mount":"F:","status":"FullyDecrypted","protection":"Off","protectors":[]},
        {"mount":"G:","status":"EncryptionInProgress","protection":"Off","protectors":["Tpm"]},
        {"mount":"H:","error":"Access is denied."}]}"#;
    let (tpm2, secure_boot, volumes) = parse_bitlocker(json).expect("parse");
    assert_eq!((tpm2, secure_boot), (Some(true), Some(true)));
    let protection = |mount: &str| {
        volumes
            .iter()
            .find(|(m, _)| m == mount)
            .map(|(_, p)| p.clone())
            .expect(mount)
    };
    let encrypted = |tpm_protector, suspended| Protection::Encrypted {
        tpm_protector,
        suspended,
        sealed_to_secure_boot: None,
    };
    assert_eq!(protection("C:"), encrypted(true, false));
    assert_eq!(protection("D:"), encrypted(true, true));
    assert_eq!(
        protection("E:"),
        encrypted(false, false),
        "a PIN needs a person at boot"
    );
    assert_eq!(protection("F:"), Protection::Unencrypted);
    assert!(matches!(protection("G:"), Protection::Unknown { .. }));
    assert!(matches!(protection("H:"), Protection::Unknown { .. }));

    // PowerShell writes a one-element array as the element, and a missing TPM as null.
    let one = r#"{"tpm_spec":null,"secure_boot":false,"volumes":{"mount":"C:","status":"FullyEncrypted","protection":"On","protectors":"Tpm"}}"#;
    let (tpm2, secure_boot, volumes) = parse_bitlocker(one).expect("parse");
    assert_eq!((tpm2, secure_boot), (Some(false), Some(false)));
    assert_eq!(volumes, vec![("C:".into(), encrypted(true, false))]);
    let tpm12 = r#"{"tpm_spec":"1.2, 2, 3","secure_boot":null,"volumes":[]}"#;
    assert_eq!(parse_bitlocker(tpm12).expect("parse").0, Some(false));
}

#[test]
fn a_compliant_machine_is_allowed() {
    assert!(compliant().findings().is_empty());
    assert_eq!(gate(&compliant(), None), Gate::Allowed);
}

#[test]
fn each_shortfall_is_a_finding_and_refuses_provisioning() {
    type Change = Box<dyn Fn(&mut EncryptionState)>;
    let cases: Vec<(Change, Finding)> = vec![
        (Box::new(|s| s.tpm2 = Some(false)), Finding::NoTpm2),
        (Box::new(|s| s.tpm2 = None), Finding::TpmUnknown),
        (
            Box::new(|s| s.secure_boot = Some(false)),
            Finding::SecureBootOff,
        ),
        (
            Box::new(|s| s.secure_boot = None),
            Finding::SecureBootUnknown,
        ),
        (
            Box::new(|s| s.volumes[1].protection = Protection::Unencrypted),
            Finding::Unencrypted(Role::Data),
        ),
        (
            Box::new(|s| {
                s.volumes[0].protection = Protection::Encrypted {
                    tpm_protector: false,
                    suspended: false,
                    sealed_to_secure_boot: None,
                }
            }),
            Finding::NoTpmProtector(Role::System),
        ),
        (
            Box::new(|s| {
                s.volumes[0].protection = Protection::Encrypted {
                    tpm_protector: true,
                    suspended: true,
                    sealed_to_secure_boot: None,
                }
            }),
            Finding::Suspended(Role::System),
        ),
        (
            Box::new(|s| {
                s.volumes[1].protection = Protection::Encrypted {
                    tpm_protector: true,
                    suspended: false,
                    sealed_to_secure_boot: Some(false),
                }
            }),
            Finding::NotSealedToSecureBoot(Role::Data),
        ),
        (
            Box::new(|s| {
                s.volumes[1].protection = Protection::Unknown {
                    reason: "not root".into(),
                }
            }),
            Finding::Unknown(Role::Data),
        ),
    ];
    for (change, expected) in cases {
        let mut state = compliant();
        change(&mut state);
        assert_eq!(state.findings(), vec![expected]);
        assert_eq!(
            gate(&state, None),
            Gate::Refused {
                findings: vec![expected]
            },
            "{expected:?}"
        );
    }
    assert_eq!(Finding::Unencrypted(Role::Data).code(), "disk-unencrypted");
    assert_eq!(Finding::Unencrypted(Role::Data).subject(), Some("data"));
    assert_eq!(Finding::NoTpm2.subject(), None);
}

#[test]
fn an_exemption_lets_a_shortfall_through_and_names_its_grant() {
    let mut state = compliant();
    state.volumes[1].protection = Protection::Unencrypted;
    let exemption = Exemption {
        grant: "UG-FDE-2026-014".into(),
    };
    assert_eq!(
        gate(&state, Some(&exemption)),
        Gate::Exempt {
            grant: "UG-FDE-2026-014".into(),
            findings: vec![Finding::Unencrypted(Role::Data)]
        }
    );
    assert_eq!(gate(&compliant(), Some(&exemption)), Gate::Allowed);
}

#[test]
fn the_state_round_trips_through_its_file() {
    let json = serde_json::to_string(&compliant()).expect("json");
    assert_eq!(
        serde_json::from_str::<EncryptionState>(&json).expect("json"),
        compliant()
    );
    assert!(json.contains(r#""state":"encrypted""#), "{json}");
}

/// The real probe on this machine: CI's runners are unencrypted VMs, so it must report, not
/// fail, and never claim compliance it cannot see.
#[test]
fn the_probe_reports_this_machine_without_failing() {
    let data = std::env::temp_dir();
    let state = probe(&system_root(), &data, at());
    assert_eq!(state.volumes.len(), 2);
    assert_eq!(state.volumes[1].path, PathBuf::from(&data));
    let findings = state.findings();
    if !cfg!(any(target_os = "linux", windows)) {
        assert!(findings.contains(&Finding::Unknown(Role::System)));
    }
}
