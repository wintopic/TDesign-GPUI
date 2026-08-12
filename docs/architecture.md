# Architecture

TDesign GPUI translates the TDesign React PC contract into native GPUI concepts
without depending on `gpui-component` or a DOM/CSS compatibility layer.

## Workspace boundaries

| Package | Responsibility |
| --- | --- |
| `tdesign-gpui` | Public components, typed state/events, theme, locale, root configuration, overlays, and prelude |
| `tdesign-gpui-assets` | Generated `IconName`, sanitized embedded SVG files, and composable `TDesignAssetSource` |
| `tdesign-gpui-story` | Non-published native gallery and component smoke application |
| `xtask` | Upstream inspection/sync, API parity generation, compatibility reports, and release checks |

## Window root and configuration

Every TDesign window should render a `TDesignRoot`. It maps React's
`ConfigProvider` to native configuration and owns the boundary at which theme,
locale, and overlay policy are resolved. `TDesignConfigState` is an entity-backed
configuration for runtime changes. Reading that entity during render lets GPUI
invalidate dependent views when the entity notifies.

`TDesignRoot` may receive an `OverlayState` that stores ordered popup, dialog,
drawer, guide, message, notification, and popconfirm entries. This keeps z-order,
Escape dismissal, modal surfaces, and window-local ownership out of individual
application views.

## Component and state mapping

- scalar and enum React props become builder methods and Rust enums;
- `TNode`/`TElement` become `AnyElement`, child elements, or named slots;
- callbacks receive typed values and, where relevant, `Window` and `App`;
- stateful controls use `Entity<XState>` and explicit event types;
- DOM-only identifiers/styles map to GPUI `ElementId`/`Styled` behavior or are
  recorded as non-applicable in parity data;
- dates and times use `chrono` values instead of strings where the value is semantic.

Native text controls implement GPUI input handling for selection, keyboard,
clipboard, and IME behavior. Composite text controls reuse `InputState` and use
internal synchronization paths to avoid duplicate public events.

## Virtualization

`ListDelegate` and `TableDelegate` defer visible-row creation to GPUI
`uniform_list`. `Tree` flattens only expanded nodes and virtualizes the resulting
visible sequence. Collection APIs should not copy or render the full data set on
each frame.

## Themes, locales, and assets

`TDesignTheme` contains typed core colors plus an extension map generated from
upstream TDesign variables. `ThemeMode::System` resolves against the native GPUI
window appearance and reapplies user overrides.

`TDesignAssetSource` serves the `tdesign/` namespace first and can fall back to
an application asset source. `IconName` contains every known upstream icon even
when `full-icons` is disabled; `contains` tells callers whether a particular SVG
was embedded in the selected feature set.

## Parity and upstream flow

`parity/component-parity.json` is generated from React PC API sources and is the
release gate for props, events, and defaults. Composite upstream families such
as Grid, Typography, Table, and Icon are merged explicitly. Mobile, miniprogram,
and UniApp-only fields are rejected from the desktop surface.

`upstream.lock.toml` records integrated commits. `cargo xtask upstream check`
classifies changes across APIs, Less/theme data, React behavior/examples/tests,
icons, and GPUI compatibility. Safe generated additions may be applied
automatically; removals, renames, behavior/API changes, and GPUI breakages require
maintainer review.

## Compatibility and release discipline

Released manifests use a crates.io GPUI version. The Zed `main` canary replaces
that dependency only in a temporary workspace configuration. Tagged releases run
formatting, tests, parity and icon checks, package verification, and publish the
asset crate before the main crate.
