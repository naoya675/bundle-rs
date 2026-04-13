use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::path::{rewrite_crate_self_refs, rewrite_line};
use crate::strip;

pub struct ModDecl {
    pub visibility: String, // "pub ", "pub(crate) ", or ""
    pub name: String,
}

/// Expand a module's source: rewrite paths, inline submodules recursively
/// `mod_depth` tracks how deep we are from the crate root (lib.rs = 0)
pub fn expand_module_source(
    source: &str,
    parent_dir: &Path,
    local_crates: &HashSet<String>,
    base_indent: &str,
    mod_depth: usize,
    remove_comments: bool,
) -> String {
    // Remove comments from this module's source before processing
    let source = if remove_comments {
        strip::remove_comments(source)
    } else {
        source.to_string()
    };

    let mut output = String::new();
    // Track inline module nesting depth within this file
    // Each entry is true if the brace opened a `mod` block
    let mut mod_brace_stack: Vec<bool> = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        // Check if this is an external mod declaration to inline
        if let Some(mod_info) = parse_external_mod(trimmed) {
            // Resolve the module file
            if let Some((mod_path, mod_content)) = resolve_module_file(parent_dir, &mod_info.name) {
                let sub_dir = if mod_path.file_name().map(|f| f == "mod.rs").unwrap_or(false) {
                    mod_path.parent().unwrap().to_path_buf()
                } else {
                    parent_dir.join(&mod_info.name)
                };

                let inline_depth: usize = mod_brace_stack.iter().filter(|&&is_mod| is_mod).count();
                let inner_indent = format!("{}    ", base_indent);
                output.push_str(&format!(
                    "{}{}mod {} {{\n",
                    base_indent, mod_info.visibility, mod_info.name
                ));
                let expanded = expand_module_source(
                    &mod_content,
                    &sub_dir,
                    local_crates,
                    &inner_indent,
                    mod_depth + inline_depth + 1,
                    remove_comments,
                );
                output.push_str(&expanded);
                output.push_str(&format!("{}}}\n", base_indent));
            }
            continue;
        }

        // Track inline module brace nesting
        let is_mod_open = is_inline_mod_open(trimmed);
        let open_braces = trimmed.chars().filter(|&c| c == '{').count();
        let close_braces = trimmed.chars().filter(|&c| c == '}').count();

        // Process closing braces first
        for _ in 0..close_braces {
            mod_brace_stack.pop();
        }
        // Process opening braces
        for i in 0..open_braces {
            // First open brace on a mod line is the mod brace
            mod_brace_stack.push(is_mod_open && i == 0);
        }

        let inline_depth: usize = mod_brace_stack.iter().filter(|&&is_mod| is_mod).count();
        let total_depth = mod_depth + inline_depth;

        // Rewrite the line
        let mut rewritten = rewrite_line(line, local_crates);

        // Rewrite `use crate::X` in submodules to `use super::...::X`
        if total_depth > 0 {
            rewritten = rewrite_crate_self_refs(&rewritten, total_depth);
        }

        if rewritten.trim().is_empty() {
            output.push('\n');
        } else {
            output.push_str(base_indent);
            output.push_str(&rewritten);
            output.push('\n');
        }
    }

    output
}

/// Check if a trimmed line opens an inline module block (e.g., `pub mod xxx {`)
fn is_inline_mod_open(trimmed: &str) -> bool {
    let s = trimmed;
    let s = s.strip_prefix("pub").map(|s| s.trim_start()).unwrap_or(s);
    let s = if s.starts_with('(') {
        if let Some(end) = s.find(')') {
            s[end + 1..].trim_start()
        } else {
            return false;
        }
    } else {
        s
    };
    if let Some(rest) = s.strip_prefix("mod") {
        let rest = rest.trim_start();
        rest.contains('{')
    } else {
        false
    }
}

/// Parse an external mod declaration, returning visibility and module name
/// Matches: `pub mod xxx;`, `pub(crate) mod xxx;`, `mod xxx;`
/// Does NOT match inline: `pub mod xxx { ... }`
fn parse_external_mod(trimmed: &str) -> Option<ModDecl> {
    let (visibility, rest) = if let Some(after_pub) = trimmed.strip_prefix("pub") {
        let after_pub = after_pub.trim_start();
        if after_pub.starts_with('(') {
            // pub(crate), pub(super), etc.
            let end = after_pub.find(')')?;
            let vis = &trimmed[..trimmed.len() - after_pub.len() + end + 1];
            (format!("{} ", vis), after_pub[end + 1..].trim_start())
        } else {
            ("pub ".to_string(), after_pub)
        }
    } else {
        ("".to_string(), trimmed)
    };

    let rest = rest.strip_prefix("mod")?.trim_start();

    // Must end with `;` and not contain `{`
    if !rest.ends_with(';') || rest.contains('{') {
        return None;
    }

    let name = rest.trim_end_matches(';').trim().to_string();
    // Validate it's an identifier
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }

    Some(ModDecl { visibility, name })
}

/// Resolve a module file: try `name.rs` then `name/mod.rs`
fn resolve_module_file(parent_dir: &Path, name: &str) -> Option<(PathBuf, String)> {
    // Try name.rs first
    let file_path = parent_dir.join(format!("{}.rs", name));
    if file_path.exists() {
        let content = fs::read_to_string(&file_path)
            .unwrap_or_else(|_| panic!("failed to read {}", file_path.display()));
        return Some((file_path, content));
    }

    // Try name/mod.rs
    let mod_path = parent_dir.join(name).join("mod.rs");
    if mod_path.exists() {
        let content = fs::read_to_string(&mod_path)
            .unwrap_or_else(|_| panic!("failed to read {}", mod_path.display()));
        return Some((mod_path, content));
    }

    None
}
