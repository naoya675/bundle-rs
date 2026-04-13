use std::collections::HashSet;

/// Rewrite a single line: handle `use` statements and macro invocations
pub fn rewrite_line(line: &str, local_crates: &HashSet<String>) -> String {
    let trimmed = line.trim();

    // Handle `use crate_name::...` -> `use crate::crate_name::...`
    if trimmed.starts_with("use ") {
        return rewrite_use_line(line, local_crates);
    }

    // Handle `crate_name::macro_name!` -> `macro_name!`
    rewrite_macro_invocation(line, local_crates)
}

/// Rewrite `use crate_name::Item` -> `use crate::crate_name::Item`
fn rewrite_use_line(line: &str, local_crates: &HashSet<String>) -> String {
    let trimmed = line.trim();
    let after_use = &trimmed["use ".len()..];

    for crate_name in local_crates {
        let underscored = crate_name.replace('-', "_");
        let prefix = format!("{}::", underscored);
        if after_use.starts_with(&prefix) {
            let indent = &line[..line.len() - line.trim_start().len()];
            let rest = &after_use[prefix.len()..];
            return format!("{}use crate::{}::{}", indent, underscored, rest);
        }
        // `use crate_name;`
        let bare = format!("{};", underscored);
        if after_use == bare {
            let indent = &line[..line.len() - line.trim_start().len()];
            return format!("{}use crate::{};", indent, underscored);
        }
    }

    line.to_string()
}

/// Rewrite `crate_name::macro_name!(...)` -> `macro_name!(...)`
fn rewrite_macro_invocation(line: &str, local_crates: &HashSet<String>) -> String {
    let mut result = line.to_string();
    for crate_name in local_crates {
        let underscored = crate_name.replace('-', "_");
        let prefix = format!("{}::", underscored);
        result = result.replace(&prefix, "");
    }
    result
}

/// Rewrite `use crate::X` to `use super::...::X` for inlined submodules
/// `depth` is the number of `super::` needed (module depth from crate root)
pub fn rewrite_crate_self_refs(line: &str, depth: usize) -> String {
    let super_chain = std::iter::repeat_n("super", depth)
        .collect::<Vec<_>>()
        .join("::");

    let mut result = line.to_string();
    result = result.replace("use crate::", &format!("use {}::", super_chain));
    result
}

/// Rewrite use paths and macro invocations in source code (for main.rs)
pub fn rewrite_source(source: &str, local_crates: &HashSet<String>) -> String {
    let mut lines: Vec<String> = Vec::new();

    for line in source.lines() {
        let rewritten = rewrite_line(line, local_crates);
        lines.push(rewritten);
    }

    let mut result = lines.join("\n");
    if source.ends_with('\n') {
        result.push('\n');
    }
    result
}
