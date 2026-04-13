# bundle-rs

A Rust source bundler for competitive programming — inlines local path dependencies into a single `.rs` file for online judge submission.

## Features

- **Dependency resolution** — extracts local `path = "..."` dependencies from `Cargo.toml` and resolves transitive dependencies recursively
- **Topological ordering** — dependencies are emitted in correct order (depended-upon crates first)
- **Submodule inlining** — `pub mod xxx;` declarations are expanded inline with their file contents (`xxx.rs` or `xxx/mod.rs`)
- **Use path rewriting** — `use crate_name::Item` becomes `use crate::crate_name::Item`
- **Macro support** — `crate_name::macro_name!(...)` is rewritten to `macro_name!(...)` for `#[macro_export]` macros
- **Unused crate exclusion** — only crates actually referenced in `main.rs` (and their transitive dependencies) are bundled
- **`#[cfg(test)]` removal** — test modules are automatically stripped
- **Comment removal** — optionally strip comments from library code while preserving `main.rs` comments
- **Compilation check** — verifies the bundled output compiles via `cargo check`
- **Formatting** — applies `rustfmt` to the final output

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
cp target/release/bundle-rs ~/.local/bin/
```

## Usage

```bash
bundle-rs [path] [options]
```

### Options

| Option | Default | Description |
|---|---|---|
| `--check` / `--no-check` | on | Compile check via `cargo check` |
| `--fmt` / `--no-fmt` | on | Format with `rustfmt --edition 2024` |
| `--remove-comments` | off | Strip comments from library sources |

### Examples

Given [library-rs/verification/library-checker/unionfind](https://github.com/naoya675/library-rs/tree/main/verification/library-checker/unionfind) with dependencies on `union-find` and `query`:

```bash
# Bundle from within the project directory
cd library-rs/verification/library-checker/unionfind
bundle-rs

# Bundle by path
bundle-rs library-rs/verification/library-checker/unionfind

# Strip comments from library sources
bundle-rs --remove-comments
```

### Output

<details>
<summary><code>bundle-rs library-rs/verification/library-checker/unionfind</code></summary>

```rust
// verification-helper: PROBLEM https://judge.yosupo.jp/problem/unionfind

use proconio::input;

use crate::union_find::UnionFind;

define_query! {
    Query {
        0 => Query0(u: usize, v: usize),
        1 => Query1(u: usize, v: usize),
    }
}

fn main() {
    input! {
        n: usize,
        q: usize,
        queries: [Query; q],
    }
    let mut uf = UnionFind::new(n);

    for query in queries {
        match query {
            Query0(u, v) => {
                uf.merge(u, v);
            }
            Query1(u, v) => {
                println!("{}", if uf.same(u, v) { 1 } else { 0 });
            }
        }
    }
}

// --- bundled ---

pub mod query {
    #[macro_export]
    macro_rules! define_query {
        // ...
    }
}

pub mod union_find {
    #[derive(Debug, Clone)]
    pub struct UnionFind {
        n: usize,
        par: Vec<usize>,
        siz: Vec<usize>,
    }

    impl UnionFind {
        pub fn new(n: usize) -> Self { /* ... */ }
        pub fn merge(&mut self, x: usize, y: usize) -> usize { /* ... */ }
        pub fn same(&mut self, x: usize, y: usize) -> bool { /* ... */ }
        pub fn leader(&mut self, x: usize) -> usize { /* ... */ }
        pub fn size(&mut self, x: usize) -> usize { /* ... */ }
        pub fn groups(&mut self) -> Vec<Vec<usize>> { /* ... */ }
    }
}
```

</details>

## How it works

1. Parse `Cargo.toml` to find local path dependencies
2. Recursively resolve transitive dependencies
3. Determine which crates are actually used in `main.rs`
4. Topologically sort the dependency graph
5. Rewrite `use` paths and macro invocations in `main.rs`
6. For each dependency, expand `lib.rs` with submodules inlined and paths rewritten
7. Strip `#[cfg(test)]` blocks
8. Optionally remove comments from library sources
9. Apply `rustfmt` and `cargo check`

## Supported `Cargo.toml` format

```toml
[package]
name = "unionfind"
version = "0.1.0"
edition = "2024"

[dependencies]
proconio = { version = "0.5.0", features = ["derive"] }  # external — kept as-is
query = { path = "../../../macro/query" }  # local — bundled
union-find = { path = "../../../data-structure/union-find" }  # local — bundled
```

External crates (without `path`) are not bundled and remain as dependencies.

## Known limitations

- `#[path = "..."]` module path attributes are not supported
- `$crate` rewriting in macros is not supported (not needed if macros don't reference types from their own crate)

## License

MIT
