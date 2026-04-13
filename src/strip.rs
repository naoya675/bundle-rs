/// Remove all comments from source code
/// Handles: //, ///, //!, /* ... */ (including nested)
/// Preserves string literals and the bundled libraries separator
pub fn remove_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut result = String::new();
    let mut i = 0;

    while i < len {
        // String literal (double quote)
        if chars[i] == '"' {
            result.push(chars[i]);
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' {
                    result.push(chars[i]);
                    i += 1;
                    if i < len {
                        result.push(chars[i]);
                        i += 1;
                    }
                } else {
                    result.push(chars[i]);
                    i += 1;
                }
            }
            if i < len {
                result.push(chars[i]); // closing "
                i += 1;
            }
            continue;
        }

        // Raw string literal (r"..." or r#"..."#)
        if chars[i] == 'r' && i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '#') {
            let start = i;
            i += 1;
            let mut hashes = 0;
            while i < len && chars[i] == '#' {
                hashes += 1;
                i += 1;
            }
            if i < len && chars[i] == '"' {
                // It's a raw string
                result.push_str(&chars[start..=i].iter().collect::<String>());
                i += 1;
                // Find closing "###
                loop {
                    if i >= len {
                        break;
                    }
                    if chars[i] == '"' {
                        let mut end_hashes = 0;
                        let mut j = i + 1;
                        while j < len && chars[j] == '#' && end_hashes < hashes {
                            end_hashes += 1;
                            j += 1;
                        }
                        if end_hashes == hashes {
                            result.push_str(&chars[i..j].iter().collect::<String>());
                            i = j;
                            break;
                        }
                    }
                    result.push(chars[i]);
                    i += 1;
                }
                continue;
            } else {
                // Not a raw string, just 'r' followed by something else
                i = start;
            }
        }

        // Character literal
        if chars[i] == '\'' {
            // Could be a char literal or a lifetime. Char literals: 'x', '\n', '\x00', '\u{...}'
            if i + 2 < len && chars[i + 1] == '\\' {
                // Escaped char literal
                result.push(chars[i]);
                i += 1;
                while i < len && chars[i] != '\'' {
                    result.push(chars[i]);
                    i += 1;
                }
                if i < len {
                    result.push(chars[i]);
                    i += 1;
                }
                continue;
            } else if i + 2 < len && chars[i + 2] == '\'' {
                // Simple char literal like 'x'
                result.push(chars[i]);
                result.push(chars[i + 1]);
                result.push(chars[i + 2]);
                i += 3;
                continue;
            }
            // Otherwise it's a lifetime or just an apostrophe, pass through
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Block comment /* ... */ (possibly nested)
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < len && depth > 0 {
                if i + 1 < len && chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < len && chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Line comment //
        if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
            // Skip until end of line
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    // Clean up: remove trailing whitespace on each line and collapse multiple blank lines
    let mut cleaned = String::new();
    let mut prev_blank = false;
    for line in result.lines() {
        let trimmed_end = line.trim_end();
        let is_blank = trimmed_end.is_empty();
        if is_blank && prev_blank {
            continue;
        }
        cleaned.push_str(trimmed_end);
        cleaned.push('\n');
        prev_blank = is_blank;
    }

    // Remove trailing blank lines
    while cleaned.ends_with("\n\n") {
        cleaned.pop();
    }

    cleaned
}

/// Remove `#[cfg(test)]` annotated items (mod blocks)
pub fn remove_cfg_test(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut result: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Detect #[cfg(test)]
        if trimmed == "#[cfg(test)]" {
            // Skip the #[cfg(test)] line and the following block
            i += 1;
            if i < lines.len() {
                let next_trimmed = lines[i].trim();
                if next_trimmed.contains('{') {
                    // Skip the entire braced block
                    let mut depth = 0;
                    loop {
                        if i >= lines.len() {
                            break;
                        }
                        for c in lines[i].chars() {
                            if c == '{' {
                                depth += 1;
                            } else if c == '}' {
                                depth -= 1;
                            }
                        }
                        i += 1;
                        if depth == 0 {
                            break;
                        }
                    }
                } else {
                    // Single item (e.g., #[cfg(test)] use ...; or fn)
                    i += 1;
                }
            }
            continue;
        }

        result.push(lines[i]);
        i += 1;
    }

    let mut out = result.join("\n");
    if source.ends_with('\n') {
        out.push('\n');
    }
    out
}
