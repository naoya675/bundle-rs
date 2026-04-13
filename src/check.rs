use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

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

pub fn run_check(source: &str, target_dir: &Path) {
    // Create a temporary cargo project to check the bundled source
    let tmp_dir = std::env::temp_dir().join("bundle_check");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(tmp_dir.join("src")).expect("failed to create temp dir");

    // Copy Cargo.toml but remove local path dependencies (they're bundled now)
    let cargo_toml_path = target_dir.join("Cargo.toml");
    let cargo_content = fs::read_to_string(&cargo_toml_path).expect("failed to read Cargo.toml");
    let mut doc: toml::Value = cargo_content.parse().expect("failed to parse Cargo.toml");

    if let Some(deps) = doc.get_mut("dependencies").and_then(|d| d.as_table_mut()) {
        // Remove all local path dependencies (they're inlined)
        let to_remove: Vec<String> = deps
            .iter()
            .filter(|(_, v)| v.get("path").is_some())
            .map(|(k, _)| k.clone())
            .collect();
        for key in to_remove {
            deps.remove(&key);
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
