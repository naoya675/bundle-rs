use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DepInfo {
    pub path: PathBuf,
    pub deps: Vec<String>,
}

/// Parse local dependencies (those with `path = "..."`) from a Cargo.toml
pub fn parse_local_deps(cargo_toml_path: &Path) -> HashMap<String, PathBuf> {
    let content = fs::read_to_string(cargo_toml_path).expect("failed to read Cargo.toml");
    let doc: toml::Value = content.parse().expect("failed to parse Cargo.toml");

    let mut deps = HashMap::new();
    if let Some(dep_table) = doc.get("dependencies").and_then(|d| d.as_table()) {
        let base_dir = cargo_toml_path.parent().unwrap();
        for (name, value) in dep_table {
            if let Some(path_str) = value.get("path").and_then(|p| p.as_str()) {
                let abs_path = base_dir.join(path_str).canonicalize().unwrap_or_else(|_| {
                    panic!(
                        "failed to resolve path for dependency '{}': {}",
                        name, path_str
                    )
                });
                deps.insert(name.clone(), abs_path);
            }
        }
    }
    deps
}

/// Parse non-path (external/crates.io) dependencies from a Cargo.toml
pub fn parse_external_deps(cargo_toml_path: &Path) -> BTreeMap<String, toml::Value> {
    let content = fs::read_to_string(cargo_toml_path).expect("failed to read Cargo.toml");
    let doc: toml::Value = content.parse().expect("failed to parse Cargo.toml");

    let mut deps = BTreeMap::new();
    if let Some(dep_table) = doc.get("dependencies").and_then(|d| d.as_table()) {
        for (name, value) in dep_table {
            let has_path = value
                .as_table()
                .map(|t| t.contains_key("path"))
                .unwrap_or(false);
            if !has_path {
                deps.insert(name.clone(), value.clone());
            }
        }
    }
    deps
}

/// Recursively resolve all transitive local dependencies using BFS
pub fn resolve_all_deps(direct_deps: &HashMap<String, PathBuf>) -> HashMap<String, DepInfo> {
    let mut all: HashMap<String, DepInfo> = HashMap::new();
    let mut queue: VecDeque<(String, PathBuf)> = VecDeque::new();

    for (name, path) in direct_deps {
        queue.push_back((name.clone(), path.clone()));
    }

    while let Some((name, path)) = queue.pop_front() {
        if all.contains_key(&name) {
            continue;
        }

        let sub_cargo_toml = path.join("Cargo.toml");
        let sub_deps = parse_local_deps(&sub_cargo_toml);

        let dep_names: Vec<String> = sub_deps.keys().cloned().collect();

        all.insert(
            name.clone(),
            DepInfo {
                path: path.clone(),
                deps: dep_names,
            },
        );

        for (sub_name, sub_path) in sub_deps {
            if !all.contains_key(&sub_name) {
                queue.push_back((sub_name, sub_path));
            }
        }
    }

    all
}

/// Topological sort: dependencies come before dependents (Kahn's algorithm)
pub fn topological_sort(deps: &HashMap<String, DepInfo>) -> Vec<String> {
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut adj: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for name in deps.keys() {
        in_degree.entry(name.clone()).or_insert(0);
        adj.entry(name.clone()).or_default();
    }

    for (name, info) in deps {
        for dep in &info.deps {
            if deps.contains_key(dep) {
                adj.entry(dep.clone()).or_default().push(name.clone());
                *in_degree.entry(name.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut queue: VecDeque<String> = VecDeque::new();
    let mut initial: Vec<String> = in_degree
        .iter()
        .filter(|&(_, &d)| d == 0)
        .map(|(n, _)| n.clone())
        .collect();
    initial.sort();
    queue.extend(initial);

    let mut result = Vec::new();
    while let Some(name) = queue.pop_front() {
        result.push(name.clone());
        let mut next: Vec<String> = Vec::new();
        if let Some(neighbors) = adj.get(&name) {
            for neighbor in neighbors {
                let deg = in_degree.get_mut(neighbor).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next.push(neighbor.clone());
                }
            }
        }
        next.sort();
        queue.extend(next);
    }

    result
}

/// Find crate names actually used in main.rs source
/// Checks for `use crate_name::` and `crate_name::` patterns
pub fn find_used_crates(
    main_source: &str,
    local_deps: &HashMap<String, PathBuf>,
    all_deps: &HashMap<String, DepInfo>,
) -> HashSet<String> {
    let mut used: HashSet<String> = HashSet::new();

    // Find directly used crates in main.rs
    for crate_name in local_deps.keys() {
        let underscored = crate_name.replace('-', "_");
        let use_prefix = format!("{}::", underscored);
        // Check for `use crate_name::`, `crate_name::macro!`, etc.
        if main_source.contains(&use_prefix) {
            used.insert(crate_name.clone());
        }
    }

    // Add transitive dependencies of used crates
    let mut queue: VecDeque<String> = used.iter().cloned().collect();
    let mut visited: HashSet<String> = used.clone();

    while let Some(name) = queue.pop_front() {
        if let Some(info) = all_deps.get(&name) {
            for dep in &info.deps {
                if !visited.contains(dep) {
                    visited.insert(dep.clone());
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    visited
}
