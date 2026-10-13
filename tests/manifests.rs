// SPDX-FileCopyrightText: 2026 Sebastien Rousseau
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The version in Cargo.toml is the single source of truth (AGENTS.md,
//! invariant 4). The registry manifests must carry the same version, or
//! a directory listing points at a release that is not the one shipped.

use serde_json::Value;

fn manifest(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn glama_json_carries_the_crate_version() {
    assert_eq!(manifest("glama.json")["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn server_json_carries_the_crate_version() {
    let server = manifest("server.json");
    assert_eq!(server["version"], env!("CARGO_PKG_VERSION"));
    for package in server["packages"].as_array().expect("packages array") {
        assert_eq!(package["version"], env!("CARGO_PKG_VERSION"));
    }
}
