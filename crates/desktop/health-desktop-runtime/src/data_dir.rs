//! The per-machine data directory (desktop-client.md § Storage and backup; ADR 0040): one per
//! PC, shared by every user's sessions on it, never in a user's profile.

use std::path::PathBuf;

/// The per-machine data directory for `app` on this platform: `%ProgramData%\<app>` on Windows,
/// `/var/lib/<app>` on Linux. macOS is deferred (ADR 0041); it is given
/// `/Library/Application Support/<app>`.
pub fn per_machine(app: &str) -> PathBuf {
    per_machine_with(app, |name| std::env::var_os(name))
}

fn per_machine_with(app: &str, var: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    if cfg!(windows) {
        var("ProgramData")
            .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from)
            .join(app)
    } else if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support").join(app)
    } else {
        PathBuf::from("/var/lib").join(app)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_directory_is_the_machine_s_not_a_user_s() {
        let dir = per_machine_with("health-desktop", |_| None);
        assert!(dir.ends_with("health-desktop"));
        let text = dir.to_string_lossy().to_lowercase();
        assert!(
            !text.contains("users") && !text.contains("/home/"),
            "{text}"
        );
        if cfg!(windows) {
            let given = per_machine_with("app", |_| Some(r"D:\ProgramData".into()));
            assert_eq!(given, PathBuf::from(r"D:\ProgramData\app"));
        }
    }
}
