## Summary

Describe the problem and the native GPUI solution.

## Upstream references

Link relevant TDesign React PC documentation/API/source and GPUI issues/source.

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets --no-default-features`
- [ ] `cargo check --workspace --all-targets --all-features`
- [ ] `cargo test --workspace`
- [ ] `cargo doc --workspace --no-deps`
- [ ] `cargo xtask parity check`
- [ ] `cargo xtask release check`

## Compatibility checklist

- [ ] Public API and parity records are aligned, or this change has no public API impact.
- [ ] Keyboard, focus, disabled state, IME/clipboard, and AccessKit implications were considered.
- [ ] Visual changes include screenshots or gallery coverage where practical.
- [ ] Windows, macOS, and Linux implications were considered.
- [ ] English and Simplified Chinese documentation remain aligned where affected.
- [ ] `CHANGELOG.md` is updated when users need migration or release information.
- [ ] No incompatible third-party code or assets were added.

## Breaking changes

List public API, MSRV, dependency, asset, behavior, or migration impact. Write “None” if not applicable.
