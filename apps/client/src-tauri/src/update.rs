//! The desktop's own updates (#136; deployment.md § Environments and rollout; desktop-client.md
//! § Updates). Sync fetches the release its roll-out records allow, from the edge it is enrolled
//! with or from central, and hands it here: it is verified against the updater key baked into
//! this build and kept under `update/` in the data directory. At the next start, before the store
//! is opened, a staged release newer than this one is verified again, a backup is taken, and the
//! installer runs.
//!
//! - A build without `HEALTH_UPDATER_PUBLIC_KEY` (a developer's) fetches no updates.
//! - An update never goes back to an older release, and a release that does not verify is
//!   deleted, never installed.
//! - With backups configured, no backup means no install: the update waits for the next start.

use std::fs;
use std::path::{Path, PathBuf};

use health_configuration::package::Version;
use health_configuration::rollout::{Artifact, Target};
use serde::{Deserialize, Serialize};

/// The updater key this build pins, as `tauri signer generate` prints it: set by the release
/// workflow, absent from a developer's build.
pub(crate) const UPDATER_KEY: Option<&str> = option_env!("HEALTH_UPDATER_PUBLIC_KEY");

/// The release this build is.
pub(crate) fn running() -> Result<Version, String> {
    env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|_| "the package version is not major.minor.patch".to_owned())
}

/// This machine's artifact.
pub(crate) const fn target() -> Target {
    if cfg!(windows) {
        Target::DesktopWindows
    } else {
        Target::DesktopLinux
    }
}

/// What `update/staged.json` records of the staged release.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Staged {
    version: String,
    artifact: Artifact,
}

/// Verifies and keeps this desktop's update, under `update/` in its data directory.
pub(crate) struct DesktopStager {
    dir: PathBuf,
    key: String,
}

impl DesktopStager {
    /// Stages under `data_dir`, verifying against `key`.
    pub(crate) fn new(data_dir: &Path, key: &str) -> Self {
        Self {
            dir: data_dir.join("update"),
            key: key.to_owned(),
        }
    }

    fn record(&self) -> PathBuf {
        self.dir.join("staged.json")
    }

    fn read(&self) -> Option<Staged> {
        let text = fs::read_to_string(self.record()).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The installer of the staged release, verified again, if it is newer than `running`.
    /// A staged release that is not newer, or no longer verifies, is cleared.
    pub(crate) fn pending(&self, running: &Version) -> Option<(Version, PathBuf)> {
        let staged = self.read()?;
        let version: Option<Version> = staged.version.parse().ok();
        let installer = self.dir.join(file_name(&staged.artifact));
        let verified = version.as_ref().is_some_and(|v| v > running)
            && fs::read(&installer).is_ok_and(|bytes| {
                health_packages::release::verify_desktop(&bytes, &staged.artifact, &self.key)
                    .is_ok()
            });
        match (verified, version) {
            (true, Some(version)) => Some((version, installer)),
            _ => {
                self.clear();
                None
            }
        }
    }

    /// Clears what is staged.
    pub(crate) fn clear(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// The artifact's file name, without any path its record could carry.
fn file_name(artifact: &Artifact) -> String {
    Path::new(&artifact.file)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("installer")
        .to_owned()
}

impl health_sync_client::releases::Stager for DesktopStager {
    fn staged(&self) -> Option<Version> {
        self.read().and_then(|staged| staged.version.parse().ok())
    }

    fn stage(&self, version: &Version, artifact: &Artifact, bytes: Vec<u8>) -> Result<(), String> {
        health_packages::release::verify_desktop(&bytes, artifact, &self.key)
            .map_err(|e| e.to_string())?;
        // One staged release at a time: a newer one replaces it.
        self.clear();
        fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let installer = self.dir.join(file_name(artifact));
        let part = installer.with_extension("part");
        fs::write(&part, &bytes).map_err(|e| format!("{}: {e}", part.display()))?;
        fs::rename(&part, &installer).map_err(|e| format!("{}: {e}", installer.display()))?;
        let record = serde_json::to_string(&Staged {
            version: version.to_string(),
            artifact: artifact.clone(),
        })
        .map_err(|e| e.to_string())?;
        // The record last: a half-written installer is never staged.
        fs::write(self.record(), record).map_err(|e| format!("staged.json: {e}"))?;
        eprintln!("health-desktop: release {version} is staged; it installs at the next start");
        Ok(())
    }
}

/// How the staged installer is run on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Install {
    /// Windows: the NSIS installer, passive (a progress bar, no questions), while this process
    /// exits so its files can be replaced.
    Installer(PathBuf),
    /// Linux, running as an AppImage at `at`: the new AppImage replaces it, and is started.
    AppImage { new: PathBuf, at: PathBuf },
    /// Linux, installed from the apt repository: apt updates it, not the desktop.
    Apt,
}

/// How `installer` is installed here; `appimage` is `$APPIMAGE`, set when running as one.
pub(crate) fn install_for(installer: PathBuf, appimage: Option<PathBuf>) -> Install {
    if cfg!(windows) {
        Install::Installer(installer)
    } else {
        match appimage {
            Some(at) => Install::AppImage { new: installer, at },
            None => Install::Apt,
        }
    }
}

/// Installs, once a backup is taken. `Ok(true)` when this process must now exit for the new
/// release to take over.
pub(crate) fn install(install: &Install, stager: &DesktopStager) -> Result<bool, String> {
    match install {
        Install::Installer(installer) => {
            std::process::Command::new(installer)
                .arg("/P")
                .spawn()
                .map_err(|e| format!("{}: {e}", installer.display()))?;
            Ok(true)
        }
        Install::AppImage { new, at } => {
            // Beside the running AppImage, then renamed over it: never half-replaced.
            let beside = at.with_extension("new");
            fs::copy(new, &beside).map_err(|e| format!("{}: {e}", beside.display()))?;
            executable(&beside)?;
            fs::rename(&beside, at).map_err(|e| format!("{}: {e}", at.display()))?;
            stager.clear();
            std::process::Command::new(at)
                .args(std::env::args_os().skip(1))
                .spawn()
                .map_err(|e| format!("{}: {e}", at.display()))?;
            Ok(true)
        }
        Install::Apt => {
            eprintln!(
                "health-desktop: a newer release is staged, but this desktop was installed from \
                 the apt repository: update it with apt"
            );
            Ok(false)
        }
    }
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(unix))]
fn executable(_: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use health_sync_client::releases::Stager as _;

    use super::*;

    // A throwaway Tauri updater key, and `an installer\n` signed with it by `tauri signer sign`
    // (2.12.4): the fixture health-packages' release tests use.
    const UPDATER: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEE2QUQxMjNEMTJFQjE4REQKUldUZEdPc1NQUkt0cGoyU1c4Q0pGMWkyVWZubzdZZTNpQmRyUzdVdG5wWWdKMVZHNVZwZ2V1ZjgK";
    const OTHER_UPDATER: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDMzODEwRUJCMkYyNzRFMjQKUldRa1RpY3Z1dzZCTXgvbnIydmpuM3pNSGZycmM1OTRHcE1PdVFQWmNkVDlpbk5KTUNrbUhqeGcK";
    const INSTALLER: &[u8] = b"an installer\n";
    const INSTALLER_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUZEdPc1NQUkt0cGlSZzhORUhNM1JiTUJrK0ZNSC9FQXZxbG5lUTJlSGNTRXBFUVhvVlhDVThMeC9KVkl1d0gwRmpDaUo0djdDNlp6TnJmTDZEMGZJbGQrdmNnbWJWcFEwPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0ODE0ODg5CWZpbGU6c2V0dXAuZXhlCms0OGVNdmQ3M3lFVEhMUGZtY2JYeVU4TEE0UVIxdXUwV0ZkWC93Z3pkZG95aXBOZTNyU2VTeVlQZGlUM2RmbWZabDVvTDExd2YrbnZtR3VoK0s4UkJRPT0K";

    fn artifact() -> Artifact {
        Artifact {
            target: target(),
            file: "../health-platform-0.2.0-x86_64.AppImage".into(),
            sha256: health_packages::release::sha256(INSTALLER),
            size: INSTALLER.len() as u64,
            signature: INSTALLER_SIG.into(),
        }
    }

    fn version(text: &str) -> Version {
        text.parse().expect("version")
    }

    #[test]
    fn a_verified_newer_release_is_staged_and_offered_at_the_next_start() {
        let dir = tempfile::tempdir().expect("dir");
        let stager = DesktopStager::new(dir.path(), UPDATER);
        assert_eq!(stager.staged(), None);
        stager
            .stage(&version("0.2.0"), &artifact(), INSTALLER.to_vec())
            .expect("staged");
        assert_eq!(stager.staged(), Some(version("0.2.0")));
        let (staged, installer) = stager.pending(&version("0.1.0")).expect("pending");
        assert_eq!(staged, version("0.2.0"));
        // Kept inside update/, whatever path the record named.
        assert_eq!(
            installer.parent(),
            Some(dir.path().join("update").as_path())
        );
        assert_eq!(fs::read(&installer).expect("installer"), INSTALLER);
    }

    #[test]
    fn a_release_that_does_not_verify_is_never_staged() {
        let dir = tempfile::tempdir().expect("dir");
        let other = DesktopStager::new(dir.path(), OTHER_UPDATER);
        assert!(
            other
                .stage(&version("0.2.0"), &artifact(), INSTALLER.to_vec())
                .is_err()
        );
        let stager = DesktopStager::new(dir.path(), UPDATER);
        assert!(
            stager
                .stage(&version("0.2.0"), &artifact(), b"tampered\n".to_vec())
                .is_err()
        );
        assert_eq!(stager.staged(), None);
        assert!(stager.pending(&version("0.1.0")).is_none());
    }

    #[test]
    fn a_staged_release_tampered_with_or_not_newer_is_cleared_not_installed() {
        let dir = tempfile::tempdir().expect("dir");
        let stager = DesktopStager::new(dir.path(), UPDATER);
        stager
            .stage(&version("0.2.0"), &artifact(), INSTALLER.to_vec())
            .expect("staged");
        // Already running it, or newer: never backwards.
        assert!(stager.pending(&version("0.2.0")).is_none());
        assert_eq!(stager.staged(), None);
        stager
            .stage(&version("0.2.0"), &artifact(), INSTALLER.to_vec())
            .expect("staged");
        let installer = dir.path().join("update").join(file_name(&artifact()));
        fs::write(&installer, b"swapped on disk\n").expect("tamper");
        assert!(stager.pending(&version("0.1.0")).is_none());
        assert!(!installer.exists());
    }

    #[test]
    fn linux_installs_by_appimage_or_leaves_it_to_apt() {
        let installer = PathBuf::from("/data/update/new.AppImage");
        if cfg!(windows) {
            assert_eq!(
                install_for(installer.clone(), None),
                Install::Installer(installer)
            );
        } else {
            assert_eq!(install_for(installer.clone(), None), Install::Apt);
            let at = PathBuf::from("/opt/health.AppImage");
            assert_eq!(
                install_for(installer.clone(), Some(at.clone())),
                Install::AppImage { new: installer, at }
            );
        }
    }
}
