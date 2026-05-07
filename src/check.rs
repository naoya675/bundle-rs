use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::deps;

pub fn run_rustfmt(source: &str) -> String {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("failed to run rustfmt: {}", e);
            std::process::exit(1);
        });

    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    if !output.status.success() {
        eprintln!(
            "rustfmt failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        // Return original source if rustfmt fails
        return source.to_string();
    }

    String::from_utf8(output.stdout).unwrap_or_else(|_| source.to_string())
}

pub fn run_check(source: &str, target_dir: &Path, local_dep_cargo_tomls: &[PathBuf]) {
    // Create a temporary cargo project to check the bundled source
    let tmp_dir = std::env::temp_dir().join("bundle_check");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(tmp_dir.join("src")).expect("failed to create temp dir");

    // Copy Cargo.toml but remove local path dependencies (they're bundled now)
    let cargo_toml_path = target_dir.join("Cargo.toml");
    let cargo_content = fs::read_to_string(&cargo_toml_path).expect("failed to read Cargo.toml");
    let mut doc: toml::Value = cargo_content.parse().expect("failed to parse Cargo.toml");

    if let Some(deps_table) = doc.get_mut("dependencies").and_then(|d| d.as_table_mut()) {
        // Remove all local path dependencies (they're inlined)
        let to_remove: Vec<String> = deps_table
            .iter()
            .filter(|(_, v)| v.get("path").is_some())
            .map(|(k, _)| k.clone())
            .collect();
        for key in to_remove {
            deps_table.remove(&key);
        }

        // Merge external dependencies from bundled local crates
        // verify-side specs take precedence; first-seen wins for inter-lib conflicts
        let mut merged_from_libs: std::collections::BTreeMap<String, toml::Value> =
            std::collections::BTreeMap::new();
        for cargo_toml in local_dep_cargo_tomls {
            let lib_externals = deps::parse_external_deps(cargo_toml);
            for (name, value) in lib_externals {
                if let Some(existing) = merged_from_libs.get(&name) {
                    if existing != &value {
                        eprintln!(
                            "warning: external dep '{}' has conflicting specs; using {}",
                            name,
                            toml::to_string(existing).unwrap_or_default().trim()
                        );
                    }
                } else {
                    merged_from_libs.insert(name, value);
                }
            }
        }
        for (name, value) in merged_from_libs {
            if !deps_table.contains_key(&name) {
                deps_table.insert(name, value);
            }
        }
    }

    let cargo_out = toml::to_string_pretty(&doc).expect("failed to serialize Cargo.toml");
    fs::write(tmp_dir.join("Cargo.toml"), cargo_out).expect("failed to write temp Cargo.toml");
    fs::write(tmp_dir.join("src/main.rs"), source).expect("failed to write temp main.rs");

    let output = Command::new("cargo")
        .args(["check", "--color", "always"])
        .current_dir(&tmp_dir)
        .output()
        .unwrap_or_else(|e| {
            eprintln!("failed to run cargo check: {}", e);
            std::process::exit(1);
        });

    // Clean up
    let _ = fs::remove_dir_all(&tmp_dir);

    if !output.status.success() {
        eprintln!(
            "--check failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(1);
    }

    eprintln!("--check passed");
}
