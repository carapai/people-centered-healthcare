//! Embeds the built web app in the edge binary (client-application.md § Serving and versions).
//!
//! The bundle is `apps/client/dist` (`bun run build`), or `HEALTH_WEB_DIST`. Without one the
//! edge builds with no app and serves only its API, so the Rust checks need no JavaScript build.
//! Source maps stay out of the binary, and the bundle's `runtime.json` is replaced by the edge's.
#![allow(
    clippy::print_stdout,
    reason = "cargo reads a build script's instructions from its stdout"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-env-changed=HEALTH_WEB_DIST");
    let dist = env::var_os("HEALTH_WEB_DIST").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../apps/client/dist"),
        PathBuf::from,
    );
    // Cargo watches a directory's whole tree, and notices one appearing.
    println!("cargo::rerun-if-changed={}", dist.display());

    let mut assets = Vec::new();
    let mut build = None;
    if dist.join("index.html").is_file() {
        collect(&dist, &dist, &mut assets);
        assets.sort();
        build = fs::read_to_string(dist.join("build.json"))
            .ok()
            .and_then(|text| build_id(&text));
    }

    let mut out = String::from("&[\n");
    for (path, file) in &assets {
        let file = file.to_str().expect("the web bundle's paths are UTF-8");
        writeln!(out, "    ({path:?}, include_bytes!({file:?})),").expect("writing to a String");
    }
    out.push(']');
    let build = match build {
        Some(id) => format!("Some({id:?})"),
        None => "None".to_owned(),
    };
    let generated = format!(
        "/// The embedded bundle's files, by URL path.\n\
         pub(super) static ASSETS: &[(&str, &[u8])] = {out};\n\
         /// The embedded bundle's build (its `build.json`).\n\
         pub(super) static BUILD: Option<&str> = {build};\n"
    );
    let target = Path::new(&env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("web.rs");
    fs::write(target, generated).expect("writing the embedded bundle's table");
}

fn collect(root: &Path, dir: &Path, assets: &mut Vec<(String, PathBuf)>) {
    let entries = fs::read_dir(dir).expect("reading the web bundle");
    for entry in entries {
        let path = entry.expect("reading the web bundle").path();
        if path.is_dir() {
            collect(root, &path, assets);
            continue;
        }
        let relative = path.strip_prefix(root).expect("inside the bundle");
        let url: Vec<_> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let url = format!("/{}", url.join("/"));
        if url.ends_with(".map") || url == "/runtime.json" || url == "/build.json" {
            continue;
        }
        let file = path.canonicalize().expect("a bundle file's absolute path");
        assets.push((url, file));
    }
}

/// `{"build":"…"}`, written by the client's Vite config; parsed by hand to keep the build script
/// free of dependencies.
fn build_id(text: &str) -> Option<String> {
    let (_, rest) = text.split_once("\"build\"")?;
    let (_, rest) = rest.split_once('"')?;
    let (id, _) = rest.split_once('"')?;
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
        .then(|| id.to_owned())
}
