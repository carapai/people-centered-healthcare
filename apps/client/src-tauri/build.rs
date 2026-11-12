//! Tauri's build step, with the app's commands listed: only these are invocable, and only as the
//! capability grants (desktop-client.md § Platforms and packaging).

fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["api_request", "sign_in", "sign_out"]),
    ))
    .unwrap_or_else(|error| panic!("tauri build: {error}"));
}
