# TDesign GPUI

<p align="center">
  Native TDesign React PC components for GPUI, written in Rust.
</p>

<p align="center">
  <a href="README.zh-CN.md">简体中文</a> · English
</p>

<p align="center">
  <a href="https://github.com/wintopic/TDesign-GPUI/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/wintopic/TDesign-GPUI/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/wintopic/TDesign-GPUI/blob/main/LICENSE-MIT"><img alt="License" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg"></a>
  <a href="https://github.com/wintopic/TDesign-GPUI/blob/main/rust-toolchain.toml"><img alt="Rust 1.96" src="https://img.shields.io/badge/rust-1.96%2B-93450a.svg?logo=rust"></a>
  <a href="https://github.com/zed-industries/zed/tree/main/crates/gpui"><img alt="GPUI 0.2.2" src="https://img.shields.io/badge/GPUI-0.2.2-0052d9.svg"></a>
  <img alt="Components: 71" src="https://img.shields.io/badge/components-71-2ba471.svg">
  <img alt="Icons: 2,354" src="https://img.shields.io/badge/icons-2%2C354-e37318.svg">
</p>

TDesign GPUI is an **unofficial community implementation** of the
[TDesign React PC](https://tdesign.tencent.com/react/getting-started) component
system for [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).
It exposes native GPUI elements, idiomatic Rust builders, typed entity state and
events, runtime theme/locale configuration, virtualized collections, native
file uploads, and the complete TDesign icon set.

> This project is not affiliated with or endorsed by Tencent, TDesign, Zed, or
> their maintainers. TDesign and GPUI remain the property of their respective
> owners. See [Third-party notices](THIRD_PARTY_NOTICES.md).

## Project status

The current release line is **0.1.x**. The repository exposes the complete
71-page React PC component surface plus a native `ConfigProvider` adapter, and
its machine-readable parity records contain no unmapped upstream fields.
However, `0.x` still means APIs may be refined, and full macOS reference-image
validation plus exhaustive AccessKit acceptance remain work toward `1.0`.

In scope:

- 71 TDesign React PC component surfaces and native `ConfigProvider` mapping
- light, dark, system, and token-overridden themes
- `zh-CN`, `en-US`, and application-provided locale messages
- all 2,354 TDesign SVG icons (`full-icons`, enabled by default)
- native GPUI desktop applications on Windows, macOS, and Linux
- daily compatibility checks against TDesign upstreams and Zed/GPUI `main`

Out of scope for this workspace: TDesign Mobile, AI Chat/Pro, miniprogram,
UniApp, and WASM.

## Installation

The initial source release is available directly from GitHub:

```toml
[dependencies]
gpui = "0.2.2"
tdesign-gpui = { git = "https://github.com/wintopic/TDesign-GPUI", branch = "main" }
```

For reproducible production builds, replace `branch` with a released `tag` or
exact `rev` when one is available. After publication to crates.io, the
dependency becomes:

```toml
[dependencies]
gpui = "0.2.2"
tdesign-gpui = "0.1"
```

The minimum supported Rust version is 1.96 and tracks GPUI's stable toolchain
requirements explicitly. Install GPUI's platform prerequisites before building;
on macOS this includes Xcode and its command-line tools, while Linux desktop
packages depend on the selected Wayland/X11 backend. See the
[GPUI README](https://github.com/zed-industries/zed/tree/main/crates/gpui) for
current upstream requirements.

## Quick start

This is a complete native window using `TDesignRoot`, `Button`, `Input`, and an
embedded icon. The same source is compiled as
[`examples/basic.rs`](crates/tdesign-gpui/examples/basic.rs).

```rust
use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
use tdesign_gpui::{
    Button, ButtonVariant, Icon, IconName, Input, InputState, TDesignAssetSource, TDesignRoot,
};

struct BasicExample {
    input: gpui::Entity<InputState>,
}

impl BasicExample {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: InputState::new(cx, ""),
        }
    }
}

impl Render for BasicExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TDesignRoot::new().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_6()
                .child("TDesign GPUI")
                .child(Input::new(self.input.clone()))
                .child(
                    Button::new("Confirm")
                        .variant(ButtonVariant::Primary)
                        .icon(Icon::new(IconName::Check)),
                ),
        )
    }
}

fn main() {
    Application::new()
        .with_assets(TDesignAssetSource::new())
        .run(|cx: &mut App| {
            tdesign_gpui::init(cx);
            cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(BasicExample::new)
            })
            .expect("open TDesign GPUI example window");
            cx.activate(true);
        });
}
```

Run it from a checkout:

```sh
cargo run -p tdesign-gpui --example basic
```

If the application already has an asset source, preserve it with
`TDesignAssetSource::with_fallback(existing_source)` or
`with_shared_fallback(...)`.

## Root configuration, theme, and locale

`TDesignRoot` is the window-level counterpart of React's `ConfigProvider`. It
resolves theme and locale globals, hosts application content, and can also own
the window's popup/dialog/drawer/message/notification stack.

```rust,no_run
use tdesign_gpui::{Locale, TDesignConfig, TDesignRoot, ThemeMode, ThemeOverrides};

fn configured_root() -> TDesignRoot {
    let config = TDesignConfig::new()
        .theme_mode(ThemeMode::System)
        .theme_overrides(ThemeOverrides::new().brand(gpui::rgb(0x7c3aed)))
        .locale(Locale::en_us())
        .message("confirm", "Save");
    TDesignRoot::with_config(config)
}
```

For runtime changes, keep a `TDesignConfigState` entity in the owning view, pass
its clone through `.config_state(...)`, and update it from a GPUI callback:

```rust,no_run
use tdesign_gpui::{Locale, TDesignConfigState};

fn switch_to_chinese(state: &gpui::Entity<TDesignConfigState>, cx: &mut gpui::App) {
    state.update(cx, |state, cx| state.set_locale(Locale::zh_cn(), cx));
}
```

Because `TDesignRoot` reads the configuration entity while rendering, GPUI
tracks the dependency automatically; `set_theme`, `set_locale`, and `update`
notify the entity and redraw the affected view on the next frame.

## State and events

Interactive controls use `Entity<XState>` instead of hidden DOM state. Inputs
share `InputState`, including keyboard editing, selection, clipboard, native
IME composition, controlled updates, and typed `InputEvent::Change` events.

```rust,no_run
use gpui::Context;
use tdesign_gpui::{InputEvent, InputState};

struct FormView {
    input: gpui::Entity<InputState>,
    value: String,
}

impl FormView {
    fn new(cx: &mut Context<Self>) -> Self {
        let input = InputState::new(cx, "");
        cx.subscribe(&input, |this, _input, event, cx| {
            let InputEvent::Change(change) = event;
            this.value = change.current.to_string();
            cx.notify();
        })
        .detach();
        Self {
            input,
            value: String::new(),
        }
    }
}
```

The same entity/event pattern is used by selection controls, date/time
controls, forms, trees, and uploads. Callbacks receive typed values plus GPUI's
`Window` and `App` contexts when interaction needs application access.

## Uploads

`Upload` uses GPUI's native file picker. Set `action` for the built-in multipart
HTTP transport, including progress, cooperative cancellation, retry, and
`UploadEvent` lifecycle notifications:

```rust,no_run
use tdesign_gpui::{Upload, UploadState};

fn upload_control(cx: &mut gpui::App) -> Upload {
    Upload::new(UploadState::new(cx))
        .action("https://example.test/upload")
        .multiple(true)
}
```

Applications with custom authentication or storage can implement
`UploadBackend` and pass an `Arc<dyn UploadBackend>` through `.backend(...)`.
Override `upload_with_request` to consume `UploadRequest`, report byte progress,
and honor `UploadCancellation`; implementing the simpler `upload` method is
enough for backends that do not expose progress.

```rust,no_run
use gpui::{App, SharedString, Task};
use std::sync::Arc;
use tdesign_gpui::{Upload, UploadBackend, UploadState};

struct MyBackend;

impl UploadBackend for MyBackend {
    fn upload(&self, path: SharedString, _cx: &mut App) -> Task<anyhow::Result<SharedString>> {
        Task::ready(Ok(format!("stored://{path}").into()))
    }
}

fn custom_upload(cx: &mut App) -> Upload {
    Upload::new(UploadState::new(cx)).backend(Arc::new(MyBackend))
}
```

This custom-backend example needs `anyhow = "1"` in the application manifest.

## Virtualized List, Table, and Tree

Large collections do not build the complete data set every frame. `List` and
`Table` request only visible rows from delegates through GPUI `uniform_list`:

```rust,no_run
use gpui::{AnyElement, App, Window, div, prelude::*};
use std::sync::Arc;
use tdesign_gpui::{List, ListDelegate};

struct Rows(Vec<String>);

impl ListDelegate for Rows {
    fn row_count(&self) -> usize {
        self.0.len()
    }

    fn render_row(&self, index: usize, _window: &mut Window, _cx: &mut App) -> AnyElement {
        div().h_8().px_3().child(self.0[index].clone()).into_any_element()
    }
}

fn list() -> List {
    List::new("records", Arc::new(Rows((0..100_000).map(|i| i.to_string()).collect())))
}
```

`TableDelegate` extends `ListDelegate` with column metadata. `Tree` flattens
only expanded nodes and virtualizes the visible node list while preserving
typed selection and keyboard traversal state.

The compile-checked [`examples/patterns.rs`](crates/tdesign-gpui/examples/patterns.rs)
combines runtime configuration, input events, a custom upload backend, List,
Table, and Tree in one native application.

## Feature flags

| Feature | Default | Purpose |
| --- | --- | --- |
| `full-icons` | yes | Embeds all 2,354 SVG icons. Without default features, only component-required core icons are embedded. |
| `serde` | no | Adds Serde support to supported public configuration, theme, locale, and icon types. |

Minimal icon build:

```toml
tdesign-gpui = { version = "0.1", default-features = false }
```

## Component coverage

| Group | Components |
| --- | --- |
| Foundations and layout | Button, Icon, Link, Typography, Divider, Grid, Layout, Space |
| Navigation and feedback | Affix, Anchor, BackTop, Breadcrumb, Dropdown, Menu, Pagination, Steps, StickyTool, Tabs, Alert, Dialog, Drawer, Guide, Message, Notification, Popconfirm, Popup |
| Inputs and forms | AutoComplete, Cascader, Checkbox, ColorPicker, DatePicker, Form, Input, InputAdornment, InputNumber, TagInput, Radio, RangeInput, Select, SelectInput, Slider, Switch, Textarea, Transfer, TimePicker, TreeSelect, Upload |
| Data display | Avatar, Badge, Calendar, Card, Collapse, Comment, Descriptions, Empty, Image, ImageViewer, List, Loading, Progress, QRCode, Skeleton, Statistic, Swiper, Table, Tag, Timeline, Tooltip, Tree, Watermark, Rate |

The source of truth is [`parity/component-parity.json`](parity/component-parity.json).
Every upstream React PC prop, event, and default is classified as a Rust method,
type, state, event, native adaptation, or explicit non-applicability. An empty,
incomplete, or unmapped record fails `cargo xtask parity check` and blocks the
release workflow.

## Upstream synchronization

[`upstream.lock.toml`](upstream.lock.toml) pins the integrated commits from
`tdesign-api`, `tdesign-common`, `tdesign-react`, `tdesign-icons`, and Zed.

```sh
cargo xtask upstream check
cargo xtask upstream sync --apply
cargo xtask parity generate
cargo xtask parity check
```

The daily workflow regenerates API, theme, icon, behavior, visual, and GPUI
compatibility reports. Additive token/icon-only changes may be labeled
`sync-safe` after checks; removals, renames, component behavior/API changes, and
GPUI breakages always produce a draft PR for maintainer review. The GPUI canary
uses a temporary dependency override and never rewrites the stable manifest.

## Development

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --no-default-features
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo doc --workspace --no-deps
cargo run -p tdesign-gpui-story
cargo xtask parity check
cargo xtask release check
```

See [Architecture](docs/architecture.md),
[migration from TDesign React](docs/migration-from-react.md), and
[CONTRIBUTING.md](CONTRIBUTING.md) before making substantial changes.

## Roadmap to 1.0

- maintain 100% machine-readable React PC API/event/default mapping
- complete fixed-reference Light/Dark visual baselines on the native macOS renderer
- meet the project SSIM and one-logical-pixel layout acceptance thresholds
- complete AccessKit role/name/value/state auditing for every interactive component
- finish bilingual component documentation and gallery state coverage
- maintain stable Windows/macOS/Linux build, test, docs, and gallery smoke checks

See [CHANGELOG.md](CHANGELOG.md) for release notes. Security issues should be
reported according to [SECURITY.md](SECURITY.md); usage questions belong in
GitHub Discussions or the channels described in [SUPPORT.md](SUPPORT.md).

## License

Original project code is dual-licensed under
[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Generated or
redistributed TDesign assets retain their upstream MIT attribution. See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for details.
