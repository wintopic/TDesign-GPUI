# Contributing to TDesign GPUI

Thank you for helping build a dependable native TDesign component system for
GPUI. Contributions are welcome as bug reports, API reviews, documentation,
tests, platform fixes, upstream synchronization work, and component code.

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
Please use [SECURITY.md](SECURITY.md), not a public issue, for vulnerabilities.

## Before opening an issue

- Search existing issues and pull requests.
- Confirm the behavior on the pinned Rust toolchain and a supported desktop OS.
- Reduce bugs to a small example when practical.
- State whether the behavior comes from TDesign React, GPUI, or this adapter.
- Include the upstream TDesign page/API field when reporting parity differences.

Use the repository's bug and feature forms so maintainers receive the versions,
platform, reproduction, expected behavior, and upstream references needed to
triage the report.

## Development setup

Requirements:

- Rust 1.96 or the toolchain pinned by `rust-toolchain.toml`
- platform prerequisites required by GPUI
- Git

Clone and verify the workspace:

```sh
git clone https://github.com/wintopic/TDesign-GPUI.git
cd TDesign-GPUI
cargo fmt --all -- --check
cargo check -p tdesign-gpui --all-targets --no-default-features --locked
cargo check -p tdesign-gpui-assets --all-targets --no-default-features --locked
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked
cargo test --workspace --locked
cargo run -p xtask --locked -- parity check
cargo run -p xtask --locked -- release check
```

Launch the native gallery with:

```sh
cargo run -p tdesign-gpui-story
```

## Repository structure

- `crates/tdesign-gpui`: public components, state, theme, locale, overlays, and prelude
- `crates/tdesign-gpui-assets`: generated icon enum, embedded SVG files, and GPUI asset source
- `crates/tdesign-gpui-story`: non-published native gallery and smoke application
- `xtask`: parity, upstream synchronization, generation, and release checks
- `parity`: machine-readable React PC component/API mappings
- `upstream`: generated review reports and synchronization metadata

The workspace `default-members` intentionally exclude the non-published story
and maintenance crates for normal library development. CI always uses
`--workspace` when it needs complete story and `xtask` coverage.

Read [docs/architecture.md](docs/architecture.md) before changing root,
state/event, overlay, collection, theme, or generation contracts.

## Change requirements

### Components and public API

- Prefer native GPUI builders and strong Rust types.
- Stateful controls should use `Entity<XState>` with explicit typed events.
- Do not expose DOM-only concepts such as CSS selectors, `className`, or arbitrary
  `data-*` props. Record the native adaptation or non-applicability in parity data.
- Keep keyboard, focus, IME, clipboard, disabled-state, and accessibility behavior
  in the same change as the relevant interactive component.
- Use delegates and GPUI virtualization for collections that can contain large data sets.
- Avoid adding `gpui-component`; this workspace intentionally depends on GPUI directly.

### Parity records

Any TDesign API change must update the machine-readable mapping. Generate after
refreshing the appropriate upstream cache, review the diff, and run:

```sh
cargo xtask parity generate
cargo xtask parity check
```

Never edit a record to claim completion without a corresponding public API,
state/event mapping, native adaptation, or documented non-applicability.

### Icons and theme data

Generated files should be produced through `xtask` or existing build scripts.
Do not hand-edit generated icon names, embedded-asset match arms, or generated
theme token tables. SVG additions must pass the sanitizer checks in the asset
build script and retain upstream attribution.

### Tests and docs

- Add `#[gpui::test]` coverage for state transitions and interaction behavior.
- Include regression tests for bugs.
- Add or update a gallery state for visible component changes.
- Keep English and Simplified Chinese user-facing documentation aligned.
- Ensure README and rustdoc examples use real public APIs and compile.

## Pull request checklist

Before opening a pull request, run:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --no-default-features
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo doc --workspace --no-deps
cargo xtask parity check
cargo xtask release check
```

A pull request should:

- explain the problem and chosen native mapping;
- link relevant TDesign and GPUI sources or issues;
- describe keyboard, focus, accessibility, and platform implications;
- include screenshots for visual changes when possible;
- call out public API, MSRV, dependency, asset, and generated-file changes;
- update the changelog when users need to know about the change.

Maintainers may ask that large API changes begin as an issue before review.
Generated upstream PRs follow the classification rules described in the README.

## Commit and licensing expectations

Focused commits with clear messages make review easier. By submitting a
contribution, you agree that your contribution is licensed under the project's
`MIT OR Apache-2.0` terms and that you have the right to provide it. Do not copy
code or assets whose license is incompatible with this repository.
