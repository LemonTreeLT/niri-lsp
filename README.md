> [!WARNING]
> Think twice before you want to contribute to this project, because it was totally written by codex  
> 当你打算为这个项目付出精力的时候请三思，因为这个项目完全是由 codex 写的
# niri-lsp

[![CI](https://github.com/LemonTreeLT/niri-lsp/actions/workflows/ci.yml/badge.svg)](https://github.com/LemonTreeLT/niri-lsp/actions/workflows/ci.yml)

`niri-lsp` is a stdio language server for [niri](https://github.com/YaLTeR/niri) KDL configuration files. This first release supports the audited schema snapshot for **niri 26.4.0**.

It needs no locally installed niri. Only a root `config.kdl` and files it recursively includes receive niri diagnostics and language features. A differently named root can opt in by making its first non-empty line `// niri-lsp: root`.

```sh
nix run github:LemonTreeLT/niri-lsp# -- --version
```

## Installation with Nix

Run the language server without installing it:

```sh
nix run github:LemonTreeLT/niri-lsp#
```

Install the default package into your user profile:

```sh
nix profile install github:LemonTreeLT/niri-lsp#default
```

### Cachix binary cache

This project publishes Linux build outputs to the public [`niri-lsp` Cachix cache](https://app.cachix.org/cache/niri-lsp#pull). To use it locally, install the Cachix client and configure the cache once:

```sh
nix run github:cachix/cachix -- use niri-lsp
```

This is optional: Nix will build the package from source when the cache is not configured or does not contain an output for your platform.

For systems without Nix, download the prebuilt x86_64 Linux archive from the [latest GitHub release](https://github.com/LemonTreeLT/niri-lsp/releases/latest). Verify its accompanying `.sha256` file before placing `niri-lsp` on your `PATH`.

For NixOS or Home Manager, add the project to your flake inputs:

```nix
inputs.niri-lsp = {
  url = "github:LemonTreeLT/niri-lsp";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then install the package declaratively in NixOS:

```nix
environment.systemPackages = [
  inputs.niri-lsp.packages.${pkgs.stdenv.hostPlatform.system}.default
];
```

Or use the same package in Home Manager:

```nix
home.packages = [
  inputs.niri-lsp.packages.${pkgs.stdenv.hostPlatform.system}.default
];
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
