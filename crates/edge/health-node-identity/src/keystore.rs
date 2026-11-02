//! The node's sealed secrets: keys that never sit on disk in the clear (security.md § Keys).
//!
//! - **Linux:** `systemd-creds encrypt`, with the host key and the TPM2 where present
//!   (`--with-key=auto`), into `/etc/credstore.encrypted/`. The unit names each credential in
//!   `LoadCredentialEncrypted=`, so systemd unseals it into the unprivileged service's
//!   `$CREDENTIALS_DIRECTORY` at every start; a missing one is not fatal. Commands run as root
//!   unseal with `systemd-creds decrypt`.
//! - **Windows:** DPAPI in machine scope, through PowerShell, into
//!   `%ProgramData%\health\credentials\`, readable by SYSTEM and Administrators only. The
//!   LocalSystem service unseals each secret itself. Machine scope binds a secret to this
//!   machine, not to the TPM chip; the BitLocker volume beneath it is TPM-bound (ADR 0048).
//!
//! Secrets cross to the tools on standard input and output, never in arguments.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A sealed-secret store.
#[derive(Debug, Clone)]
pub struct Keystore {
    /// Where sealed secrets are kept.
    store: PathBuf,
    /// Where the service manager left them unsealed (`$CREDENTIALS_DIRECTORY`), if anywhere.
    loaded: Option<PathBuf>,
}

/// Why a secret could not be sealed or unsealed. Never carries the secret.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeystoreError {
    /// A secret by that name is already sealed: a node's identity is never replaced in place.
    #[error("{0} is already sealed")]
    Exists(String),
    /// The name is not a plain credential name.
    #[error("{0:?} is not a credential name")]
    Name(String),
    /// The platform tool failed or is missing.
    #[error("{0}")]
    Tool(String),
    /// This platform has no keystore.
    #[error("no keystore on this platform")]
    Unsupported,
}

impl Keystore {
    /// The system store, with the service manager's unsealed credentials when running as the
    /// service.
    pub fn system() -> Self {
        let store = if cfg!(windows) {
            let data = std::env::var_os("ProgramData").unwrap_or_else(|| r"C:\ProgramData".into());
            PathBuf::from(data).join("health").join("credentials")
        } else {
            PathBuf::from("/etc/credstore.encrypted")
        };
        Self {
            store,
            loaded: std::env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from),
        }
    }

    /// A store at `store`, with unsealed credentials in `loaded`.
    pub fn at(store: impl Into<PathBuf>, loaded: Option<PathBuf>) -> Self {
        Self {
            store: store.into(),
            loaded,
        }
    }

    /// Where `name` is sealed.
    pub fn path(&self, name: &str) -> PathBuf {
        self.store.join(name)
    }

    /// Whether `name` is sealed here, or unsealed for the service.
    pub fn contains(&self, name: &str) -> bool {
        self.loaded.as_ref().is_some_and(|d| d.join(name).is_file()) || self.path(name).is_file()
    }

    /// Seals `secret` as `name`. Refuses to replace an existing one.
    pub fn seal(&self, name: &str, secret: &[u8]) -> Result<(), KeystoreError> {
        check_name(name)?;
        let path = self.path(name);
        if path.exists() {
            return Err(KeystoreError::Exists(name.to_owned()));
        }
        std::fs::create_dir_all(&self.store)
            .map_err(|e| KeystoreError::Tool(format!("cannot create the store: {e}")))?;
        if cfg!(windows) {
            let sealed = powershell(DPAPI_PROTECT, &base64(secret))?;
            write_private(&path, sealed.trim().as_bytes())?;
            restrict(&path)
        } else if cfg!(target_os = "linux") {
            let target = path.to_str().ok_or(KeystoreError::Unsupported)?;
            let name_arg = format!("--name={name}");
            tool(
                "systemd-creds",
                &["encrypt", &name_arg, "--with-key=auto", "-", target],
                secret,
            )
            .map(drop)
        } else {
            Err(KeystoreError::Unsupported)
        }
    }

    /// The secret sealed as `name`; `None` if there is none.
    pub fn unseal(&self, name: &str) -> Result<Option<Vec<u8>>, KeystoreError> {
        check_name(name)?;
        if let Some(loaded) = &self.loaded
            && let Ok(secret) = std::fs::read(loaded.join(name))
        {
            return Ok(Some(secret));
        }
        let path = self.path(name);
        if !path.is_file() {
            return Ok(None);
        }
        if cfg!(windows) {
            let sealed = std::fs::read_to_string(&path)
                .map_err(|e| KeystoreError::Tool(format!("cannot read {name}: {e}")))?;
            let plain = powershell(DPAPI_UNPROTECT, sealed.trim())?;
            unbase64(plain.trim()).map(Some)
        } else if cfg!(target_os = "linux") {
            let source = path.to_str().ok_or(KeystoreError::Unsupported)?;
            let name_arg = format!("--name={name}");
            tool("systemd-creds", &["decrypt", &name_arg, source, "-"], &[]).map(Some)
        } else {
            Err(KeystoreError::Unsupported)
        }
    }
}

/// Credential names: what systemd accepts, kept to a portable subset.
fn check_name(name: &str) -> Result<(), KeystoreError> {
    let valid = !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(KeystoreError::Name(name.to_owned()))
    }
}

const DPAPI_PROTECT: &str = r"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$bytes = [Convert]::FromBase64String([Console]::In.ReadToEnd().Trim())
$sealed = [Security.Cryptography.ProtectedData]::Protect($bytes, $null, [Security.Cryptography.DataProtectionScope]::LocalMachine)
[Convert]::ToBase64String($sealed)
";

const DPAPI_UNPROTECT: &str = r"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$sealed = [Convert]::FromBase64String([Console]::In.ReadToEnd().Trim())
$bytes = [Security.Cryptography.ProtectedData]::Unprotect($sealed, $null, [Security.Cryptography.DataProtectionScope]::LocalMachine)
[Convert]::ToBase64String($bytes)
";

fn powershell(script: &str, input: &str) -> Result<String, KeystoreError> {
    let out = tool(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", script],
        input.as_bytes(),
    )?;
    String::from_utf8(out).map_err(|_| KeystoreError::Tool("PowerShell wrote non-UTF-8".into()))
}

/// Runs `program`, writing `input` to its standard input; its standard output on success.
fn tool(program: &str, args: &[&str], input: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| KeystoreError::Tool(format!("{program} could not run: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input)
            .map_err(|e| KeystoreError::Tool(format!("{program}: {e}")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| KeystoreError::Tool(format!("{program}: {e}")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(KeystoreError::Tool(format!(
            "{program} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn write_private(path: &Path, contents: &[u8]) -> Result<(), KeystoreError> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(contents))
        .map_err(|e| KeystoreError::Tool(format!("cannot write {}: {e}", path.display())))
}

/// SYSTEM and Administrators only, with nothing inherited.
fn restrict(path: &Path) -> Result<(), KeystoreError> {
    let target = path.to_str().ok_or(KeystoreError::Unsupported)?;
    tool(
        "icacls",
        &[
            target,
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:F",
            "/grant:r",
            "*S-1-5-32-544:F",
        ],
        &[],
    )
    .map(drop)
}

fn base64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unbase64(text: &str) -> Result<Vec<u8>, KeystoreError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|_| KeystoreError::Tool("PowerShell returned malformed base64".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_are_plain() {
        for good in ["health.facility-intermediate.key", "a_b-1"] {
            assert!(check_name(good).is_ok(), "{good}");
        }
        for bad in ["", "../x", "a/b", ".hidden", "a b", r"a\b"] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_service_reads_what_systemd_unsealed_and_never_replaces_a_secret() {
        let dir = tempfile::tempdir().expect("dir");
        let loaded = dir.path().join("credentials");
        std::fs::create_dir_all(&loaded).expect("dir");
        std::fs::write(loaded.join("health.k"), b"secret").expect("write");
        let store = Keystore::at(dir.path().join("store"), Some(loaded));
        assert!(store.contains("health.k"));
        assert_eq!(
            store.unseal("health.k").expect("unseal"),
            Some(b"secret".to_vec())
        );
        assert_eq!(store.unseal("health.absent").expect("unseal"), None);

        std::fs::create_dir_all(dir.path().join("store")).expect("dir");
        std::fs::write(store.path("health.sealed"), b"sealed").expect("write");
        assert_eq!(
            store.seal("health.sealed", b"new"),
            Err(KeystoreError::Exists("health.sealed".into()))
        );
    }

    /// The real platform keystore, where it can run unprivileged: DPAPI on Windows.
    #[test]
    #[cfg(windows)]
    fn dpapi_seals_and_unseals_in_machine_scope() {
        let dir = tempfile::tempdir().expect("dir");
        let store = Keystore::at(dir.path(), None);
        let secret = b"-----BEGIN PRIVATE KEY-----\nMIGH\n-----END PRIVATE KEY-----\n";
        store.seal("health.test.key", secret).expect("seal");
        let sealed = std::fs::read(store.path("health.test.key")).expect("sealed");
        assert!(!sealed.windows(5).any(|w| w == b"BEGIN"), "sealed at rest");
        assert_eq!(
            store.unseal("health.test.key").expect("unseal"),
            Some(secret.to_vec())
        );
    }
}
