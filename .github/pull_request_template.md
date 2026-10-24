## Summary & Intent

<!--
Explain high-level what this change achieves, why it was made, and link the related issue.
Example: Closes #123
-->

Closes #

---

## What It Changes

<!--
List the exact crates, modules, and public APIs modified or introduced.
-->
- `crates/...`: 
- `src/...`: 

---

## What It Brings

<!--
Describe the new capabilities, bug fixes, or architectural improvements introduced by this PR.
-->

---

## Possible Regressions & Edge Cases

<!--
CRITICAL FOR LLM CONTRIBUTIONS:
Analyze potential regressions, breaking changes, or subtle edge cases:
- Are there any API breaking changes?
- Are there cross-platform considerations (Linux vs. macOS vs. Windows)?
- Does this behave correctly inside Flatpak or restricted sandboxes?
- Performance or memory implications?
-->

---

## Tests Done to Ensure Stability

<!--
Describe how this was verified. Include new unit tests, integration tests, or mock servers.
-->
- [ ] Unit tests added / updated in `tests` modules
- [ ] Tested against mock server / memory backends
- [ ] Devcontainer quality gates verified

---

## Contributor / LLM Quality Checklist

- [ ] **Build**: `cargo build` passes with zero errors
- [ ] **Tests**: `cargo test --workspace` passes all tests
- [ ] **Format**: `cargo fmt --all -- --check` produces no diff
- [ ] **Lints**: `cargo clippy --all-targets -- -D warnings` reports zero warnings
- [ ] **Docs**: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` succeeds
- [ ] **Safety**: `#![forbid(unsafe_code)]` preserved (no unreviewed `unsafe`)
- [ ] **No Panics**: No `unwrap()`, `expect()`, or `panic!()` in library paths
- [ ] **Confidentiality**: Credentials, tokens, keys, and secrets are strictly redacted in `Debug` and `Display`
