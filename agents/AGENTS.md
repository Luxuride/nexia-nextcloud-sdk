# nextcloud-rust-sdk — repo rules

Rules for agents working in this repository. This file is canonical; the
root-level `AGENTS.md`, `CLAUDE.md` and `GEMINI.md` are symlinks to it.
Edit here only.

## What this repo is

A Rust client for the Nextcloud APIs, split into a workspace of small,
focused crates. Every crate's public surface is treated as an API contract,
not a scratchpad.

## Structure

- Split the project into multiple libraries wherever a boundary exists.
  Separate concerns do not share a crate: transport, auth, wire types, and
  each API area are candidates for their own crate in a workspace.
- Prefer a Cargo workspace (`crates/*`) over one growing package as soon as
  a second concern appears. Add a crate instead of a large module.
- Use structs for data, traits for behaviour, and generics for variation.
  Concrete types behind traits, not `match` on variants or `dyn` where a
  generic parameter suffices.
- No long single-file solutions. A file that needs scrolling to navigate is
  split: one type or one concern per file, `mod.rs`/`lib.rs` only re-exports
  and wiring.
- Every non-trivial solution lands as several files with a clear seam
  between them, not as one large file with sections.

## Toolchain

- Build, test, format and lint inside the devcontainer
  (`mcr.microsoft.com/devcontainers/rust:1`), never on the host.
- Edition 2024, stable Rust. Do not bump the MSRV without being asked.

## Commands

```sh
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

All four must pass before a change is considered done.

## Code rules

- No `unsafe` without an explicit request and a safety comment explaining
  the invariant it relies on.
- No `unwrap()` / `expect()` / `panic!()` on paths reachable from library
  code. Return `Result` with a typed error; the error enum is part of the
  public API.
- Public items need a doc comment. Hidden or internal items do not, unless
  the intent is non-obvious.
- Missing docs are denied: keep `#![deny(missing_docs)]` in `src/lib.rs`
  satisfied.
- Errors must name the operation that failed and preserve the source error
  (`#[source]` / `Error::source`); never stringify away the cause.
- Credentials (tokens, app passwords, basic-auth headers) must never be
  logged, formatted via `Debug`, or embedded in error messages. Debug impls
  that touch credentials must redact them.

## Dependencies

- Prefer the standard library and already-present dependencies over adding
  new crates. Justify any new dependency in the change description.
- `serde` + `serde_json` for wire formats is expected; keep wire types
  separate from public ergonomic types where they differ.
- No `*` or floating version requirements; use caret versions.

## API design

- Model Nextcloud's endpoints path-style (`/ocs/v2.php/...`,
  `/remote.php/dav/...`), one crate or module per API area.
- Express shared behaviour as traits with generic bounds so implementations
  compose; keep trait objects only for genuinely dynamic dispatch.
- Return `Result<T, Error>` from every fallible call; never silently drop a
  non-2xx response.
- Prefer owned, `Send + 'static` futures so callers can spawn them.
- Breaking changes to public items require a changelog note.

## Tests

- Unit tests live beside the code (`mod tests`). Integration tests live in
  `tests/`.
- Network behaviour is tested against a mock server, not a live Nextcloud
  instance. Never make a test require credentials.
- Every bug fix lands with a test that fails before it.

## Style

- Comments explain intent, invariants and non-obvious constraints. Never
  narrate edits, the diff, or the task.
- Keep modules small and named after the Nextcloud API area they wrap.
- Rustfmt decides formatting; do not hand-align or fight it.
