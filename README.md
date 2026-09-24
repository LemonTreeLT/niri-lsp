# niri-lsp

`niri-lsp` is a stdio language server for [niri](https://github.com/YaLTeR/niri) KDL configuration files. This first release supports the audited schema snapshot for **niri 26.4.0**.

It needs no locally installed niri. Only a root `config.kdl` and files it recursively includes receive niri diagnostics and language features. A differently named root can opt in by making its first non-empty line `// niri-lsp: root`.

```sh
nix develop       # Rust development shell
nix build         # build the package
nix run .# -- --version
```

## Neovim

Native Neovim configuration:

```lua
vim.lsp.config("niri_lsp", {
  cmd = { "niri-lsp" },
  filetypes = { "kdl" },
  root_markers = { "config.kdl" },
})
vim.lsp.enable("niri_lsp")
```

With `nvim-lspconfig`:

```lua
require("lspconfig.configs").niri_lsp = {
  default_config = { cmd = { "niri-lsp" }, filetypes = { "kdl" }, root_dir = require("lspconfig.util").root_pattern("config.kdl") },
}
require("lspconfig").niri_lsp.setup({})
```

Includes accept relative, absolute, and `~/` paths, and `optional=true`. The server reports unavailable includes, cycles, and overly deep graphs; go-to-definition and references work for included files.

## Updating niri support

1. Change the upstream reference and run `cargo xtask schema update --niri-ref <tag-or-commit>`.
2. Review the committed schema artifact and extend its validation coverage.
3. Run `cargo test`, `cargo clippy -- -D warnings`, and `nix flake check`.
4. Update the supported version in this README and `NIRI_REF`.
