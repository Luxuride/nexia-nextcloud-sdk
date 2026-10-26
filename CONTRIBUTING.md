# Contributing to nextcloud-rust-sdk

Thank you for your interest in contributing to the Nextcloud Rust SDK!

This codebase is structured as a high-integrity Cargo workspace of small, focused crates where every crate's public interface is treated as a strict API contract.

Both **human** and **LLM / AI-assisted contributions** are welcome!

**This SDK is generic.** It must not acknowledge a particular application — no
consumer's name, config format, UI state or workaround belongs in this workspace.
Protocol behaviour, runtime, VFS and journaling belong here because they are hard
and shared; presentation and product concerns belong to consumers. A contribution
shaped around one downstream client will be asked to become a general capability
or to move out. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §7.

---

## 1. Toolchain & Quality Gates

All development, testing, formatting, and linting must be executed in stable Rust (Edition 2024), inside the project devcontainer (`mcr.microsoft.com/devcontainers/rust:1`).

Before submitting any Pull Request, all five commands must pass with zero errors
and zero warnings:

```sh
cargo build
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

---

## 2. Core Code Rules

1. **No `unsafe` code**: `#![forbid(unsafe_code)]` is enforced across all crates unless explicitly requested with an accompanying safety comment explaining the invariant it relies on.
2. **Never panic in library code**: No `unwrap()`, `expect()`, or `panic!()` on paths reachable from library code. Return typed `Result<T, Error>`.
3. **Missing documentation is denied**: Every public struct, enum, function, method, trait, and module must have a clear doc comment (`#![deny(missing_docs)]`).
4. **Credential confidentiality**: Tokens, app passwords, basic-auth headers, and encryption keys must never be logged, formatted via `Debug` or `Display`, or embedded in error messages. Redact credentials (e.g. `[REDACTED]`) and wipe secret buffers on drop.
5. **Typed, causal errors**: Errors must name the operation that failed and preserve the source error via `#[source]` / `Error::source`. Never stringify away causes.
6. **Workspace crate boundaries**: Separate concerns do not share a crate. Transport, auth, storage, wire types, and distinct API areas each have their own focused crate.
7. **No single-file monoliths**: Split files by type or concern. `lib.rs` / `mod.rs` only handles re-exports and wiring.
8. **No consumer coupling**: The SDK is a general Nextcloud client. It never names
   an application, exposes a flag that exists for one client, or depends on a
   downstream product's config format or UI state.
9. **Docs move with the code**: A change to a public signature, handle command,
   `OrchestratorEvent` variant, crate layout, storage path or keyring schema
   updates the matching file in `docs/` in the same PR.

---

## 3. Requirements for LLM / AI-Assisted Contributions

Autonomous AI agents and contributors using LLMs (Claude, Gemini, GPT, etc.) are first-class contributors to this repository. To maintain high code quality and architectural integrity, **all LLM-assisted Pull Requests MUST use the PR template** and provide the following structured details:

### Required PR Sections:
1. **What it changes**:
   - Exact list of crates and modules modified or created.
   - Specific architectural boundary touched.
2. **What it brings (Motivation & Value)**:
   - What feature, fix, or capability this PR adds.
   - Relevant issue reference (`Closes #123`).
3. **Possible regressions & edge cases**:
   - Any potential breaking changes or API modifications.
   - Cross-platform implications (Linux vs. macOS vs. Windows).
   - Flatpak sandbox or container isolation constraints.
4. **Tests & stability verification**:
   - List of unit and integration tests added or executed.
   - Confirmation that all 5 repo gate commands (§1) passed cleanly in the devcontainer.
5. **Security & credential checklist**:
   - Explicit confirmation that no credentials are logged or exposed in `Debug`.
   - Confirmation of `#![forbid(unsafe_code)]`.

---

## 4. Pull Request Workflow

1. **Create an Issue**: Check existing issues or open a new one with the label `feature`, `bug`, or `chore`. Do not prefix issue titles with `feat:`.
2. **Branch naming**: Use `feature/<name>`, `fix/<name>`, or `chore/<name>`.
3. **Commit convention**: Follow Conventional Commits:
   - `feat(nextcloud-sdk-keyring): add multiplatform keyring`
   - `fix(nextcloud-dav): handle multistatus 404 cleanly`
   - `chore: update dependencies`
4. **Submit PR**: Fill out the Pull Request template completely. Ensure CI checks pass across Linux, macOS, and Windows.

---

## 5. Releasing

Releases are automated via [release-plz](https://github.com/release-plz/release-plz).

1. Follow **Conventional Commits** (`feat:`, `fix:`, `chore:`, etc.) for all changes.
2. When changes land on `main`, the `Release` workflow runs `release-plz`:
   - Evaluates conventional commits and computes the next semantic version.
   - Automatically maintains and updates an open **Release PR** containing version bumps in `Cargo.toml` and updates to `CHANGELOG.md`.
3. When the Release PR is reviewed and merged into `main`:
   - `release-plz` detects the version bump.
   - Creates the git tag (`v<version>`).
   - Publishes each workspace crate to crates.io in topological dependency order.
   - Generates the GitHub Release with changelog notes.

The workflow requires `CARGO_REGISTRY_TOKEN` configured as a repository secret and GitHub Actions workflow write permissions for contents and pull requests.
