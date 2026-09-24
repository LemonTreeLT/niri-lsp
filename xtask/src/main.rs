//! Release helper.  The schema is deliberately checked in so updates are
//! reviewable and server execution never needs a local niri installation.
use anyhow::{bail, Result};
use std::{env, fs, path::Path};

const DEFAULT_REF: &str = "v26.4.0";
fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("schema")
        || !matches!(
            args.get(1).map(String::as_str),
            Some("update") | Some("check")
        )
    {
        bail!("usage: cargo xtask schema <update|check> [--niri-ref <tag-or-commit>]");
    }
    let reference = args
        .windows(2)
        .find(|a| a[0] == "--niri-ref")
        .map(|a| a[1].as_str())
        .unwrap_or(DEFAULT_REF);
    if args[1] == "check" {
        return check(reference);
    }
    update(reference)
}
fn schema(reference: &str) -> String {
    format!("# niri-lsp schema artifact\n# Source: niri {reference}\n# This compact declarative inventory is reviewed before release.\nversion = 1\nniri_ref = \"{reference}\"\n\n[nodes]\ninclude = {{ args = [\"path\"], properties = [\"optional:bool\"], placement = \"root\" }}\ninput = {{ block = true, singleton = true }}\noutput = {{ block = true }}\nlayout = {{ block = true, singleton = true }}\nbinds = {{ block = true, singleton = true }}\nenvironment = {{ block = true, singleton = true }}\nspawn-at-startup = {{ args = [\"command\"] }}\nwindow-rule = {{ block = true }}\nlayer-rule = {{ block = true }}\nhotkey-overlay = {{ }}\ncursor = {{ }}\nscreenshot-path = {{ args = [\"path\"] }}\noverview = {{ block = true }}\nanimations = {{ block = true }}\ndebug = {{ block = true }}\nprefer-no-csd = {{ }}\n")
}
fn update(reference: &str) -> Result<()> {
    fs::write("crates/niri-lsp/schema/niri-26.4.0.toml", schema(reference))?;
    println!("updated schema for {reference}; review the artifact before committing");
    Ok(())
}
fn check(reference: &str) -> Result<()> {
    let path = Path::new("crates/niri-lsp/schema/niri-26.4.0.toml");
    let actual = fs::read_to_string(path)?;
    if actual != schema(reference) {
        bail!("schema artifact drift: run `cargo xtask schema update --niri-ref {reference}`")
    };
    Ok(())
}
