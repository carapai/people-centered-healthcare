//! The Tauri desktop client's runtime (desktop-client.md; ADRs 0036, 0039, 0040).
//!
//! - [`ipc`]: the webview's `api_request` envelope dispatched in-process into the same Axum
//!   router the edge serves over HTTP, with `tower::ServiceExt::oneshot`: no socket, and the same
//!   contract.
//! - [`data_dir`]: the per-machine data directory every user's session on the PC shares.
//! - [`keystore`]: the OS keystore port. Secrets never reach JavaScript.
//!
//! The Tauri binary (`apps/client/src-tauri`) composes these with the embedded store and the
//! application crates.

pub mod data_dir;
pub mod ipc;
pub mod keystore;
