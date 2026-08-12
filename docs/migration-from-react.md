# Migrating from TDesign React

TDesign GPUI preserves component intent and desktop behavior while adopting
native Rust and GPUI ownership patterns. It is not a JSX or DOM compatibility
layer.

## Common mappings

| TDesign React | TDesign GPUI |
| --- | --- |
| `<ConfigProvider>` | `TDesignRoot` + `TDesignConfig` or `TDesignConfigState` |
| JSX children / `TNode` | `.child(...)`, `AnyElement`, or a named builder slot |
| boolean/number/string props | typed builder methods |
| string unions | Rust enums |
| `value` + `onChange` | `Entity<XState>`, typed setters/events, and component callbacks |
| CSS class/style/container selector | GPUI `Styled`, `ElementId`, `TDesignRoot`, or explicit non-applicability |
| `Date`/formatted strings | `chrono::NaiveDate`, `NaiveTime`, or related chrono values |
| browser upload `action` | native file picker + built-in GPUI HTTP backend |
| custom upload request | `Arc<dyn UploadBackend>` |
| large array rendered with `.map()` | `ListDelegate`, `TableDelegate`, or virtualized `Tree` state |

## Example: Button

React:

```tsx
<Button theme="primary" icon={<CheckIcon />} onClick={save}>
  Save
</Button>
```

Rust:

```rust,no_run
use tdesign_gpui::{Button, ButtonVariant, Icon, IconName};

let button = Button::new("Save")
    .variant(ButtonVariant::Primary)
    .icon(Icon::new(IconName::Check))
    .on_click(|_event, _window, _cx| {
        // Save application state.
    });
```

## Example: controlled Input

React keeps the value in component/application state:

```tsx
<Input value={name} onChange={setName} />
```

GPUI keeps it in an entity. The same entity is rendered by `Input` and emits
typed changes for keyboard, clipboard, IME, and programmatic updates:

```rust,no_run
use tdesign_gpui::{Input, InputState};

fn input(cx: &mut gpui::App) -> Input {
    let state = InputState::new(cx, "initial value");
    Input::new(state)
}
```

Subscribe to `InputEvent` in the owning GPUI view when another part of the
application needs to mirror or validate the value.

## Example: ConfigProvider

React:

```tsx
<ConfigProvider globalConfig={{ classPrefix: "t", locale: enConfig }}>
  <App />
</ConfigProvider>
```

Rust:

```rust,no_run
use tdesign_gpui::{Locale, TDesignConfig, TDesignRoot, ThemeMode};

let root = TDesignRoot::with_config(
    TDesignConfig::new()
        .locale(Locale::en_us())
        .theme_mode(ThemeMode::System),
);
```

DOM class prefixes do not apply. Window-level theme, locale, overlays, focus,
and z-order are native responsibilities of `TDesignRoot` and GPUI.

## Finding exact API parity

Use `parity/component-parity.json` for the authoritative component/prop/event
mapping. Each record identifies the Rust method/type/state/event, native
adaptation, or non-applicability. If a React PC API has no complete record, it is
a release-blocking bug rather than an undocumented implicit mapping.
