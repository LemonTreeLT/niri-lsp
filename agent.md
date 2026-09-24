# Agent notes

- Use the flake as the source of truth: run Rust commands with `nix develop path:. -c …` when Cargo is not already on `PATH`.
- Keep the lockfile compatible with nixpkgs 25.05's Rust 1.86. Do not run an unconstrained dependency update; `url` currently requires `idna 1.0.3` and `idna_adapter 1.0.0` to avoid Rust 1.88-only ICU crates.
- Validate changes with `cargo fmt --check`, `cargo test --workspace --offline`, `cargo clippy --workspace --offline -- -D warnings`, `cargo xtask schema check`, and `nix flake check path:.`.
- The checked-in schema is the runtime contract. Schema changes must be produced with `cargo xtask schema update --niri-ref …`, reviewed, and tested.
- Keep `niri-lsp` stdio-only and restrict niri language features to `config.kdl` roots or files opted in with `// niri-lsp: root`, plus their include graph.
