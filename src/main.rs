mod check;
mod deps;
mod expand;
mod path;
mod strip;

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

struct Args {
    target_dir: PathBuf,
    check: bool,
    fmt: bool,
    remove_comments: bool,
}

fn parse_args() -> Args {
    let args = std::env::args().skip(1);
    let mut check = true;
    let mut fmt = true;
    let mut remove_comments = false;
    let mut target_dir: Option<PathBuf> = None;

    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            "--no-check" => check = false,
            "--fmt" => fmt = true,
            "--no-fmt" => fmt = false,
            "--remove-comments" => remove_comments = true,
            _ if arg.starts_with('-') => {
                eprintln!("unknown option: {}", arg);
                std::process::exit(1);
            }
            _ => {
                if target_dir.is_some() {
                    eprintln!("unexpected argument: {}", arg);
                    std::process::exit(1);
                }
                target_dir = Some(PathBuf::from(arg));
            }
        }
    }

    let target_dir = target_dir
        .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

    let target_dir = target_dir.canonicalize().unwrap_or_else(|_| {
        eprintln!("directory not found: {}", target_dir.display());
        std::process::exit(1);
    });

    Args {
        target_dir,
        check,
        fmt,
        remove_comments,
    }
}

fn main() {
    let args = parse_args();
    let target_dir = &args.target_dir;

    let cargo_toml_path = target_dir.join("Cargo.toml");
    let main_rs_path = target_dir.join("src/main.rs");

    if !cargo_toml_path.exists() {
        eprintln!("Cargo.toml not found in {}", target_dir.display());
        std::process::exit(1);
    }
    if !main_rs_path.exists() {
        eprintln!("src/main.rs not found in {}", target_dir.display());
        std::process::exit(1);
    }

    // Parse local dependencies from Cargo.toml
    let local_deps = deps::parse_local_deps(&cargo_toml_path);

    // Recursively resolve all transitive dependencies
    let all_deps = deps::resolve_all_deps(&local_deps);

    // Read main.rs
    let main_content = fs::read_to_string(&main_rs_path).expect("failed to read src/main.rs");

    // Find actually used crates and filter deps
    let used_crates = deps::find_used_crates(&main_content, &local_deps, &all_deps);
    let filtered_deps: std::collections::HashMap<String, deps::DepInfo> = all_deps
        .into_iter()
        .filter(|(name, _)| used_crates.contains(name))
        .collect();

    // Topological sort (dependencies first)
    let sorted = deps::topological_sort(&filtered_deps);

    // Collect all local crate names for use-path rewriting
    let local_crate_names: HashSet<String> = filtered_deps.keys().cloned().collect();

    // Rewrite main.rs
    let main_rewritten = path::rewrite_source(&main_content, &local_crate_names);

    // Build the bundled output into a String
    let mut output = main_rewritten.clone();

    if !sorted.is_empty() {
        // Ensure blank line before separator
        if !output.ends_with("\n\n") {
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push('\n');
        }
        output.push_str("// --- bundled ---\n\n");
    }

    // Output each dependency's lib.rs wrapped in pub mod
    for crate_name in &sorted {
        let dep_info = &filtered_deps[crate_name];
        let src_dir = dep_info.path.join("src");
        let lib_rs_path = src_dir.join("lib.rs");
        let lib_content = fs::read_to_string(&lib_rs_path)
            .unwrap_or_else(|_| panic!("failed to read {}", lib_rs_path.display()));

        let mod_name = crate_name.replace('-', "_");
        output.push_str(&format!("pub mod {} {{\n", mod_name));
        let expanded = expand::expand_module_source(
            &lib_content,
            &src_dir,
            &local_crate_names,
            "    ",
            0,
            args.remove_comments,
        );
        output.push_str(&expanded);
        output.push_str("}\n\n");
    }

    // Remove #[cfg(test)] blocks
    output = strip::remove_cfg_test(&output);

    // Apply rustfmt if requested
    if args.fmt {
        output = check::run_rustfmt(&output);
    }

    // Check compilation if requested
    if args.check {
        check::run_check(&output, target_dir);
    }

    print!("{}", output);
}
