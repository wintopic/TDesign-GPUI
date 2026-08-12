//! Composite form controls with strongly typed entity state.

use crate::{Input, InputEvent, InputState, SelectOption, TreeNode, ValueChange};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Hsla, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

type StringHandler = Arc<dyn Fn(&str, &mut Window, &mut App)>;

/// State for query-driven suggestions.
#[derive(Debug)]
pub struct AutoCompleteState {
    /// Current query.
    pub query: SharedString,
    /// Native text editing state used for keyboard, clipboard, and IME input.
    pub input: Entity<InputState>,
    /// Candidate options.
    pub options: Vec<SelectOption>,
    /// Highlighted suggestion index.
    pub highlighted: Option<usize>,
    /// Whether suggestions are visible.
    pub open: bool,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl AutoCompleteState {
    /// Creates autocomplete state.
    pub fn new(cx: &mut App, options: Vec<SelectOption>) -> Entity<Self> {
        let input = InputState::new(cx, "");
        cx.new(|cx| {
            let subscription = cx.subscribe(
                &input,
                |state: &mut AutoCompleteState,
                 _input: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<AutoCompleteState>| {
                    let InputEvent::Change(change) = event;
                    state.apply_query(change.current.clone(), cx);
                },
            );
            subscription.detach();
            let focus_handle = input.read(cx).focus_handle.clone();
            Self {
                query: SharedString::default(),
                input,
                options,
                highlighted: None,
                open: false,
                disabled: false,
                focus_handle,
            }
        })
    }

    /// Updates the query and opens matching suggestions.
    pub fn set_query(
        &mut self,
        query: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let query = query.into();
        let change = self.apply_query(query.clone(), cx);
        if self.input.read(cx).value != query {
            self.input.update(cx, |input, cx| {
                input.sync_value(query, cx);
            });
        }
        change
    }

    fn apply_query(
        &mut self,
        query: SharedString,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let previous = self.query.clone();
        self.query = query;
        self.open = !self.disabled;
        let query = self.query.to_lowercase();
        self.highlighted = self.options.iter().position(|option| {
            !option.disabled && (query.is_empty() || option.label.to_lowercase().contains(&query))
        });
        if previous != self.query {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.query.clone(),
        }
    }

    /// Selects an enabled option.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(option) = self
            .options
            .iter()
            .find(|option| option.key == key && !option.disabled)
        else {
            return false;
        };
        let label = option.label.clone();
        self.query = label.clone();
        if self.input.read(cx).value != label {
            self.input.update(cx, |input, cx| {
                input.sync_value(label, cx);
            });
        }
        self.open = false;
        self.highlighted = self
            .options
            .iter()
            .position(|candidate| candidate.key == key);
        cx.notify();
        true
    }

    /// Opens the suggestion popup.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            let query = self.query.to_lowercase();
            self.highlighted = self.options.iter().position(|option| {
                !option.disabled
                    && (query.is_empty() || option.label.to_lowercase().contains(&query))
            });
        }
        cx.notify();
    }

    /// Closes the suggestion popup.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Moves the highlighted matching suggestion, wrapping around enabled items.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let query = self.query.to_lowercase();
        let matching = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                (!option.disabled
                    && (query.is_empty() || option.label.to_lowercase().contains(&query)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return;
        }
        let was_open = self.open;
        self.open(cx);
        if !was_open {
            self.highlighted = if delta < 0 {
                matching.last().copied()
            } else {
                matching.first().copied()
            };
            cx.notify();
            return;
        }
        let current = self
            .highlighted
            .and_then(|index| matching.iter().position(|candidate| *candidate == index))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(matching.len() as i32) as usize;
        self.highlighted = Some(matching[next]);
        cx.notify();
    }

    /// Activates the highlighted matching suggestion.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let index = self.highlighted?;
        let key = self.options.get(index)?.key.clone();
        self.select(&key, cx).then_some(key)
    }
}

/// Query field with a native suggestion popup.
#[derive(IntoElement)]
pub struct AutoComplete {
    state: Entity<AutoCompleteState>,
    placeholder: SharedString,
    on_select: Option<StringHandler>,
}

impl AutoComplete {
    /// Creates an autocomplete control.
    pub fn new(state: Entity<AutoCompleteState>) -> Self {
        Self {
            state,
            placeholder: "请输入".into(),
            on_select: None,
        }
    }

    /// Sets empty-query placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Registers option selection.
    pub fn on_select(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for AutoComplete {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input_entity = self.state.read(cx).input.clone();
        let placeholder = self.placeholder.clone();
        let disabled = self.state.read(cx).disabled;
        let _ = input_entity.update(cx, |input, cx| {
            let changed = input.disabled != disabled || input.placeholder != placeholder;
            input.disabled = disabled;
            input.placeholder = placeholder.clone();
            if changed {
                cx.notify();
            }
        });
        let state = self.state.read(cx);
        let query = state.query.clone();
        let open = state.open;
        let highlighted = state.highlighted;
        let focus = state.focus_handle.clone();
        let options = state
            .options
            .iter()
            .filter(|option| {
                query.is_empty() || option.label.to_lowercase().contains(&query.to_lowercase())
            })
            .cloned()
            .collect::<Vec<_>>();
        let toggle_entity = self.state.clone();
        let key_entity = self.state.clone();
        let select_entity = self.state.clone();
        let key_handler = self.on_select.clone();
        let handler = self.on_select;
        let suggestions = options
            .into_iter()
            .map(|option| {
                let key = option.key.clone();
                let entity = select_entity.clone();
                let handler = handler.clone();
                let is_highlighted = entity
                    .read(cx)
                    .options
                    .iter()
                    .position(|candidate| candidate.key == key)
                    == highlighted;
                div()
                    .id(SharedString::from(format!("autocomplete-{key}")))
                    .px_3()
                    .py_2()
                    .when(is_highlighted, |this| this.bg(gpui::rgb(0xe8f1ff)))
                    .when(option.disabled, |this| this.opacity(0.5))
                    .when(!option.disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xf3f3f3)))
                    })
                    .child(option.label)
                    .on_click(move |_, window, cx| {
                        let selected = entity.update(cx, |state, cx| state.select(&key, cx));
                        if selected {
                            if let Some(handler) = &handler {
                                handler(&key, window, cx);
                            }
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-auto-complete", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(180.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-auto-complete-input", &self.state))
                    .relative()
                    .w_full()
                    .child(Input::new(input_entity.clone()))
                    .child(
                        div()
                            .absolute()
                            .right_2()
                            .top_2()
                            .text_color(gpui::rgb(0x666666))
                            .child("⌄"),
                    )
                    .on_click(move |_, window, cx| {
                        if toggle_entity.read(cx).disabled {
                            return;
                        }
                        toggle_entity.read(cx).focus_handle.focus(window);
                        toggle_entity.update(cx, |state, cx| {
                            if state.open {
                                state.close(cx);
                            } else {
                                state.open(cx);
                            }
                        });
                    }),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .right_0()
                        .mt_1()
                        .id("tdesign-auto-complete-options")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(suggestions),
                )
            })
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "down" | "arrowdown" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                    }
                    "up" | "arrowup" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                    }
                    "enter" | "space" => {
                        cx.stop_propagation();
                        let selected = key_entity.update(cx, |state, cx| {
                            if state.open {
                                state.select_highlighted(cx)
                            } else {
                                state.open(cx);
                                None
                            }
                        });
                        if let Some(selected) = selected
                            && let Some(handler) = &key_handler
                        {
                            handler(&selected, window, cx);
                        }
                    }
                    "escape" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.close(cx));
                    }
                    _ => {}
                }
            })
    }
}

/// AutoComplete component module.
pub mod auto_complete {
    pub use super::{AutoComplete, AutoCompleteState};
}

/// One hierarchical Cascader option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CascaderOption {
    /// Stable option key.
    pub key: String,
    /// Display label.
    pub label: SharedString,
    /// Nested options.
    pub children: Vec<Self>,
    /// Disabled state.
    pub disabled: bool,
}

impl CascaderOption {
    /// Creates a cascader option.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            children: Vec::new(),
            disabled: false,
        }
    }

    /// Appends a child option.
    pub fn child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }
}

/// Cascader path and popup state.
#[derive(Debug)]
pub struct CascaderState {
    /// Root options.
    pub options: Vec<CascaderOption>,
    /// Selected key path.
    pub path: Vec<String>,
    /// Key currently highlighted for keyboard activation.
    pub highlighted: Option<String>,
    /// Popup visibility.
    pub open: bool,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl CascaderState {
    /// Creates empty-path cascader state.
    pub fn new(cx: &mut App, options: Vec<CascaderOption>) -> Entity<Self> {
        cx.new(|cx| Self {
            options,
            path: Vec::new(),
            highlighted: None,
            open: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Selects a key and records its full path.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        fn find_path(options: &[CascaderOption], key: &str, path: &mut Vec<String>) -> bool {
            for option in options {
                path.push(option.key.clone());
                if option.key == key && !option.disabled {
                    return true;
                }
                if !option.disabled && find_path(&option.children, key, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        if find_path(&self.options, key, &mut path) {
            self.path = path;
            self.highlighted = Some(key.to_owned());
            self.open = false;
            cx.notify();
            true
        } else {
            false
        }
    }

    /// Opens the cascader popup.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            self.highlighted = first_enabled_cascader_key(&self.options);
        }
        cx.notify();
    }

    /// Closes the popup without changing the selected path.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Moves the keyboard highlight through enabled options in depth-first order.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let mut keys = Vec::new();
        collect_enabled_cascader_keys(&self.options, &mut keys);
        if keys.is_empty() {
            return;
        }
        let was_open = self.open;
        self.open(cx);
        if !was_open {
            self.highlighted = if delta < 0 {
                keys.last().cloned()
            } else {
                keys.first().cloned()
            };
            cx.notify();
            return;
        }
        let current = self
            .highlighted
            .as_ref()
            .and_then(|key| keys.iter().position(|candidate| candidate == key))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(keys.len() as i32) as usize;
        self.highlighted = Some(keys[next].clone());
        cx.notify();
    }

    /// Selects the highlighted option.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<Vec<String>> {
        let key = self.highlighted.clone()?;
        self.select(&key, cx).then_some(self.path.clone())
    }
}

fn first_enabled_cascader_key(options: &[CascaderOption]) -> Option<String> {
    let mut keys = Vec::new();
    collect_enabled_cascader_keys(options, &mut keys);
    keys.into_iter().next()
}

fn collect_enabled_cascader_keys(options: &[CascaderOption], keys: &mut Vec<String>) {
    for option in options {
        if option.disabled {
            continue;
        }
        keys.push(option.key.clone());
        collect_enabled_cascader_keys(&option.children, keys);
    }
}

/// Hierarchical single-value selector.
#[derive(IntoElement)]
pub struct Cascader {
    state: Entity<CascaderState>,
    placeholder: SharedString,
    on_change: Option<Arc<dyn Fn(&[String], &mut Window, &mut App)>>,
}

impl Cascader {
    /// Creates a cascader.
    pub fn new(state: Entity<CascaderState>) -> Self {
        Self {
            state,
            placeholder: "请选择".into(),
            on_change: None,
        }
    }

    /// Sets placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Registers path changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[String], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Cascader {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        fn flatten(
            options: &[CascaderOption],
            depth: usize,
            out: &mut Vec<(usize, CascaderOption)>,
        ) {
            for option in options {
                out.push((depth, option.clone()));
                flatten(&option.children, depth + 1, out);
            }
        }
        fn label_for(options: &[CascaderOption], key: &str) -> Option<SharedString> {
            for option in options {
                if option.key == key {
                    return Some(option.label.clone());
                }
                if let Some(label) = label_for(&option.children, key) {
                    return Some(label);
                }
            }
            None
        }

        let state = self.state.read(cx);
        let path = state.path.clone();
        let label = if path.is_empty() {
            self.placeholder
        } else {
            path.iter()
                .filter_map(|key| label_for(&state.options, key))
                .map(|label| label.to_string())
                .collect::<Vec<_>>()
                .join(" / ")
                .into()
        };
        let open = state.open;
        let highlighted = state.highlighted.clone();
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let mut flattened = Vec::new();
        flatten(&state.options, 0, &mut flattened);
        let toggle_entity = self.state.clone();
        let select_entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let handler = self.on_change;
        let entries = flattened
            .into_iter()
            .map(|(depth, option)| {
                let key = option.key.clone();
                let entity = select_entity.clone();
                let handler = handler.clone();
                let is_highlighted = highlighted.as_deref() == Some(key.as_str());
                div()
                    .id(SharedString::from(format!("cascader-{key}")))
                    .pl(px(12. + depth as f32 * 16.))
                    .pr_3()
                    .py_2()
                    .when(is_highlighted, |this| this.bg(gpui::rgb(0xe8f1ff)))
                    .when(option.disabled, |this| this.opacity(0.5))
                    .when(!option.disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xf3f3f3)))
                    })
                    .child(option.label)
                    .on_click(move |_, window, cx| {
                        let selected = entity.update(cx, |state, cx| state.select(&key, cx));
                        if selected && let Some(handler) = &handler {
                            let path = entity.read(cx).path.clone();
                            handler(&path, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-cascader", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(180.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-cascader-trigger", &self.state))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_8()
                    .px_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .bg(gpui::white())
                    .child(label)
                    .child("⌄")
                    .on_click(move |_, window, cx| {
                        if toggle_entity.read(cx).disabled {
                            return;
                        }
                        toggle_entity.read(cx).focus_handle.focus(window);
                        toggle_entity.update(cx, |state, cx| {
                            if state.open {
                                state.close(cx)
                            } else {
                                state.open(cx)
                            }
                        });
                    }),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt_1()
                        .id("tdesign-cascader-options")
                        .min_w(px(220.))
                        .max_h(px(280.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(entries),
                )
            })
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "down" | "arrowdown" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                    }
                    "up" | "arrowup" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                    }
                    "enter" | "space" => {
                        cx.stop_propagation();
                        let path = key_entity.update(cx, |state, cx| {
                            if state.open {
                                state.select_highlighted(cx)
                            } else {
                                state.open(cx);
                                None
                            }
                        });
                        if let Some(path) = path
                            && let Some(handler) = &key_handler
                        {
                            handler(&path, window, cx);
                        }
                    }
                    "escape" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.close(cx));
                    }
                    _ => {}
                }
            })
    }
}

/// Cascader component module.
pub mod cascader {
    pub use super::{Cascader, CascaderOption, CascaderState};
}

/// State for color selection.
#[derive(Debug)]
pub struct ColorPickerState {
    /// Selected color.
    pub color: Hsla,
    /// Popup visibility.
    pub open: bool,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl ColorPickerState {
    /// Creates color picker state.
    pub fn new(cx: &mut App, color: Hsla) -> Entity<Self> {
        cx.new(|cx| Self {
            color,
            open: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Updates the selected color.
    pub fn set_color(&mut self, color: Hsla, cx: &mut Context<Self>) -> ValueChange<Hsla> {
        let previous = self.color;
        self.color = color;
        self.open = false;
        cx.notify();
        ValueChange {
            previous,
            current: color,
        }
    }
}

/// Color swatch and preset popup.
#[derive(IntoElement)]
pub struct ColorPicker {
    state: Entity<ColorPickerState>,
    presets: Vec<Hsla>,
    on_change: Option<Arc<dyn Fn(ValueChange<Hsla>, &mut Window, &mut App)>>,
}

impl ColorPicker {
    /// Creates a picker with the TDesign functional palette.
    pub fn new(state: Entity<ColorPickerState>) -> Self {
        Self {
            state,
            presets: vec![
                gpui::rgb(0x0052d9).into(),
                gpui::rgb(0x2ba471).into(),
                gpui::rgb(0xe37318).into(),
                gpui::rgb(0xd54941).into(),
                gpui::rgb(0x7b61ff).into(),
            ],
            on_change: None,
        }
    }

    /// Replaces preset colors.
    pub fn presets(mut self, presets: impl IntoIterator<Item = Hsla>) -> Self {
        self.presets = presets.into_iter().collect();
        self
    }

    /// Registers color changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<Hsla>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for ColorPicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let color = state.color;
        let open = state.open;
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let toggle_entity = self.state.clone();
        let set_entity = self.state.clone();
        let handler = self.on_change;
        let swatches = self
            .presets
            .into_iter()
            .enumerate()
            .map(|(index, preset)| {
                let entity = set_entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("color-preset-{index}")))
                    .size_6()
                    .rounded_sm()
                    .bg(preset)
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .on_click(move |_, window, cx| {
                        let event = entity.update(cx, |state, cx| state.set_color(preset, cx));
                        if let Some(handler) = &handler {
                            handler(event, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-color-picker", &self.state))
            .track_focus(&focus)
            .relative()
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-color-picker-trigger", &self.state))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h_8()
                    .px_2()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .child(div().size_5().rounded_sm().bg(color))
                    .child(format!("{color:?}"))
                    .on_click(move |_, window, cx| {
                        if toggle_entity.read(cx).disabled {
                            return;
                        }
                        toggle_entity.read(cx).focus_handle.focus(window);
                        let _ = toggle_entity.update(cx, |state, cx| {
                            state.open = !state.open;
                            cx.notify();
                        });
                    }),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt_1()
                        .flex()
                        .gap_2()
                        .p_3()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(swatches),
                )
            })
    }
}

/// ColorPicker component module.
pub mod color_picker {
    pub use super::{ColorPicker, ColorPickerState};
}

/// Form layout direction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FormLayout {
    /// Labels and controls stack vertically.
    #[default]
    Vertical,
    /// Fields flow horizontally.
    Inline,
}

/// Validation and submission state shared by form fields.
#[derive(Debug)]
pub struct FormState {
    /// Validation messages keyed by field name.
    pub errors: BTreeMap<String, SharedString>,
    /// Whether an asynchronous submission is active.
    pub submitting: bool,
    /// Disabled state for the whole form.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl FormState {
    /// Creates empty form state.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            errors: BTreeMap::new(),
            submitting: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Sets or replaces a field error.
    pub fn set_error(
        &mut self,
        field: impl Into<String>,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.errors.insert(field.into(), message.into());
        cx.notify();
    }

    /// Clears one field error.
    pub fn clear_error(&mut self, field: &str, cx: &mut Context<Self>) {
        self.errors.remove(field);
        cx.notify();
    }
}

/// One labeled native form control.
pub struct FormItem {
    name: String,
    label: SharedString,
    required: bool,
    control: AnyElement,
}

impl FormItem {
    /// Creates a form item.
    pub fn new(
        name: impl Into<String>,
        label: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            required: false,
            control: control.into_any_element(),
        }
    }

    /// Marks the field as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

/// Native form layout and validation presenter.
#[derive(IntoElement)]
pub struct Form {
    state: Entity<FormState>,
    layout: FormLayout,
    items: Vec<FormItem>,
    submit_label: Option<SharedString>,
    on_submit: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
}

impl Form {
    /// Creates an empty form.
    pub fn new(state: Entity<FormState>) -> Self {
        Self {
            state,
            layout: FormLayout::Vertical,
            items: Vec::new(),
            submit_label: None,
            on_submit: None,
        }
    }

    /// Sets field layout.
    pub fn layout(mut self, layout: FormLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Adds one field.
    pub fn item(mut self, item: FormItem) -> Self {
        self.items.push(item);
        self
    }

    /// Adds a submit control with the given label.
    pub fn submit_label(mut self, label: impl Into<SharedString>) -> Self {
        self.submit_label = Some(label.into());
        self
    }

    /// Registers submission.
    pub fn on_submit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Form {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let errors = state.errors.clone();
        let disabled = state.disabled;
        let submitting = state.submitting;
        let focus = state.focus_handle.clone();
        let fields = self
            .items
            .into_iter()
            .map(|item| {
                let error = errors.get(&item.name).cloned();
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w(px(180.))
                    .child(
                        div()
                            .text_sm()
                            .child(if item.required { "* " } else { "" })
                            .child(item.label),
                    )
                    .child(item.control)
                    .children(error.map(|message| {
                        div()
                            .text_sm()
                            .text_color(gpui::rgb(0xd54941))
                            .child(message)
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let submit = self.submit_label.map(|label| {
            let handler = self.on_submit;
            div()
                .id(entity_id("tdesign-form-submit", &self.state))
                .flex()
                .items_center()
                .justify_center()
                .h_8()
                .px_4()
                .rounded_sm()
                .bg(gpui::rgb(0x0052d9))
                .text_color(gpui::white())
                .when(disabled || submitting, |this| this.opacity(0.5))
                .child(if submitting {
                    SharedString::from("…")
                } else {
                    label
                })
                .on_click(move |_, window, cx| {
                    if !disabled && !submitting {
                        if let Some(handler) = &handler {
                            handler(window, cx);
                        }
                    }
                })
        });
        div()
            .id(entity_id("tdesign-form", &self.state))
            .track_focus(&focus)
            .flex()
            .gap_4()
            .when(self.layout == FormLayout::Vertical, |this| this.flex_col())
            .when(self.layout == FormLayout::Inline, |this| {
                this.items_end().flex_wrap()
            })
            .when(disabled, |this| this.opacity(0.6))
            .children(fields)
            .children(submit)
    }
}

/// Form component module.
pub mod form {
    pub use super::{Form, FormItem, FormLayout, FormState};
}

/// Input slot wrapper with prefix and suffix adornments.
#[derive(IntoElement)]
pub struct InputAdornment {
    input: AnyElement,
    prefix: Option<AnyElement>,
    suffix: Option<AnyElement>,
}

impl InputAdornment {
    /// Creates an adornment wrapper.
    pub fn new(input: impl IntoElement) -> Self {
        Self {
            input: input.into_any_element(),
            prefix: None,
            suffix: None,
        }
    }

    /// Sets the leading slot.
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// Sets the trailing slot.
    pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
        self.suffix = Some(suffix.into_any_element());
        self
    }
}

impl RenderOnce for InputAdornment {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("tdesign-input-adornment")
            .flex()
            .items_center()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .overflow_hidden()
            .children(self.prefix.map(|prefix| {
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .bg(gpui::rgb(0xf3f3f3))
                    .border_r_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .child(prefix)
            }))
            .child(div().flex_1().child(self.input))
            .children(self.suffix.map(|suffix| {
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .bg(gpui::rgb(0xf3f3f3))
                    .border_l_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .child(suffix)
            }))
    }
}

/// InputAdornment component module.
pub mod input_adornment {
    pub use super::InputAdornment;
}

/// State for editable tag collections.
#[derive(Debug)]
pub struct TagInputState {
    /// Committed tags.
    pub tags: Vec<SharedString>,
    /// Uncommitted input text.
    pub draft: SharedString,
    /// Native editor for the uncommitted tag.
    pub input: Entity<InputState>,
    /// Optional maximum tag count.
    pub max: Option<usize>,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl TagInputState {
    /// Creates tag input state.
    pub fn new(cx: &mut App, tags: Vec<SharedString>) -> Entity<Self> {
        let input = InputState::new(cx, "");
        cx.new(|cx| {
            let subscription = cx.subscribe(
                &input,
                |state: &mut TagInputState,
                 _input: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<TagInputState>| {
                    let InputEvent::Change(change) = event;
                    state.draft = change.current.clone();
                    cx.notify();
                },
            );
            subscription.detach();
            let focus_handle = input.read(cx).focus_handle.clone();
            Self {
                tags,
                draft: SharedString::default(),
                input,
                max: None,
                disabled: false,
                focus_handle,
            }
        })
    }

    /// Adds a unique non-empty tag.
    pub fn push(&mut self, tag: impl Into<SharedString>, cx: &mut Context<Self>) -> bool {
        let tag = tag.into();
        if self.disabled
            || tag.is_empty()
            || self.tags.contains(&tag)
            || self.max.is_some_and(|max| self.tags.len() >= max)
        {
            return false;
        }
        self.tags.push(tag);
        self.draft = SharedString::default();
        if !self.input.read(cx).value.is_empty() {
            self.input.update(cx, |input, cx| {
                input.sync_value("", cx);
            });
        }
        cx.notify();
        true
    }

    /// Commits the current draft as a tag.
    pub fn commit_draft(&mut self, cx: &mut Context<Self>) -> bool {
        let draft = self.draft.trim().to_owned();
        self.push(draft, cx)
    }

    /// Removes a tag by index.
    pub fn remove(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        if self.disabled || index >= self.tags.len() {
            return false;
        }
        self.tags.remove(index);
        cx.notify();
        true
    }
}

/// Multi-value tag editor.
#[derive(IntoElement)]
pub struct TagInput {
    state: Entity<TagInputState>,
    placeholder: SharedString,
    on_change: Option<Arc<dyn Fn(&[SharedString], &mut Window, &mut App)>>,
}

impl TagInput {
    /// Creates a tag input.
    pub fn new(state: Entity<TagInputState>) -> Self {
        Self {
            state,
            placeholder: "请输入并确认".into(),
            on_change: None,
        }
    }

    /// Sets placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Registers tag collection changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for TagInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input_entity = self.state.read(cx).input.clone();
        let disabled = self.state.read(cx).disabled;
        let placeholder = self.placeholder.clone();
        let _ = input_entity.update(cx, |input, cx| {
            let changed = input.disabled != disabled || input.placeholder != placeholder;
            input.disabled = disabled;
            input.placeholder = placeholder.clone();
            if changed {
                cx.notify();
            }
        });
        let state = self.state.read(cx);
        let tags = state.tags.clone();
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let handler = self.on_change;
        let chips = tags
            .into_iter()
            .enumerate()
            .map(|(index, tag)| {
                let entity = entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("tag-input-{index}")))
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(gpui::rgb(0xf3f3f3))
                    .child(tag)
                    .child("×")
                    .on_click(move |_, window, cx| {
                        let removed = entity.update(cx, |state, cx| state.remove(index, cx));
                        if removed {
                            if let Some(handler) = &handler {
                                let tags = entity.read(cx).tags.clone();
                                handler(&tags, window, cx);
                            }
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-tag-input", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .flex_wrap()
            .gap_1()
            .min_h(px(32.))
            .min_w(px(220.))
            .px_2()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .bg(gpui::white())
            .when(disabled, |this| this.opacity(0.5))
            .children(chips)
            .child(
                div()
                    .flex_1()
                    .min_w(px(96.))
                    .child(Input::new(input_entity.clone())),
            )
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        let changed = key_entity.update(cx, |state, cx| state.commit_draft(cx));
                        if changed && let Some(handler) = &key_handler {
                            let tags = key_entity.read(cx).tags.clone();
                            handler(&tags, window, cx);
                        }
                    }
                    "backspace" if key_entity.read(cx).draft.is_empty() => {
                        let index = key_entity.read(cx).tags.len().checked_sub(1);
                        if let Some(index) = index {
                            cx.stop_propagation();
                            let changed =
                                key_entity.update(cx, |state, cx| state.remove(index, cx));
                            if changed && let Some(handler) = &key_handler {
                                let tags = key_entity.read(cx).tags.clone();
                                handler(&tags, window, cx);
                            }
                        }
                    }
                    _ => {}
                }
            })
    }
}

/// TagInput component module.
pub mod tag_input {
    pub use super::{TagInput, TagInputState};
}

/// State for a pair of text values.
#[derive(Debug)]
pub struct RangeInputState {
    /// Start value.
    pub start: SharedString,
    /// End value.
    pub end: SharedString,
    /// Native editor for the start endpoint.
    pub start_input: Entity<InputState>,
    /// Native editor for the end endpoint.
    pub end_input: Entity<InputState>,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl RangeInputState {
    /// Creates range input state.
    pub fn new(
        cx: &mut App,
        start: impl Into<SharedString>,
        end: impl Into<SharedString>,
    ) -> Entity<Self> {
        let start = start.into();
        let end = end.into();
        let start_input = InputState::new(cx, start.clone());
        let end_input = InputState::new(cx, end.clone());
        cx.new(|cx| {
            let start_subscription = cx.subscribe(
                &start_input,
                |state: &mut RangeInputState,
                 _input: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<RangeInputState>| {
                    let InputEvent::Change(change) = event;
                    state.start = change.current.clone();
                    cx.notify();
                },
            );
            start_subscription.detach();
            let end_subscription = cx.subscribe(
                &end_input,
                |state: &mut RangeInputState,
                 _input: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<RangeInputState>| {
                    let InputEvent::Change(change) = event;
                    state.end = change.current.clone();
                    cx.notify();
                },
            );
            end_subscription.detach();
            let focus_handle = start_input.read(cx).focus_handle.clone();
            Self {
                start,
                end,
                start_input,
                end_input,
                disabled: false,
                focus_handle,
            }
        })
    }

    /// Sets both endpoints.
    pub fn set(
        &mut self,
        start: impl Into<SharedString>,
        end: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.start = start.into();
        self.end = end.into();
        let start = self.start.clone();
        let end = self.end.clone();
        if self.start_input.read(cx).value != start {
            self.start_input.update(cx, |input, cx| {
                input.sync_value(start, cx);
            });
        }
        if self.end_input.read(cx).value != end {
            self.end_input.update(cx, |input, cx| {
                input.sync_value(end, cx);
            });
        }
        cx.notify();
    }

    /// Swaps endpoints.
    pub fn swap(&mut self, cx: &mut Context<Self>) {
        std::mem::swap(&mut self.start, &mut self.end);
        let start = self.start.clone();
        let end = self.end.clone();
        self.start_input.update(cx, |input, cx| {
            input.sync_value(start, cx);
        });
        self.end_input.update(cx, |input, cx| {
            input.sync_value(end, cx);
        });
        cx.notify();
    }
}

/// Two coordinated text inputs with a separator.
#[derive(IntoElement)]
pub struct RangeInput {
    state: Entity<RangeInputState>,
    separator: SharedString,
}

impl RangeInput {
    /// Creates a range input.
    pub fn new(state: Entity<RangeInputState>) -> Self {
        Self {
            state,
            separator: "—".into(),
        }
    }

    /// Sets separator content.
    pub fn separator(mut self, separator: impl Into<SharedString>) -> Self {
        self.separator = separator.into();
        self
    }
}

impl RenderOnce for RangeInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let start_input = state.start_input.clone();
        let end_input = state.end_input.clone();
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        for input in [&start_input, &end_input] {
            let _ = input.update(cx, |input, cx| {
                if input.disabled != disabled {
                    input.disabled = disabled;
                    cx.notify();
                }
            });
        }
        div()
            .id(entity_id("tdesign-range-input", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .h_8()
            .min_w(px(240.))
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .bg(gpui::white())
            .when(disabled, |this| this.opacity(0.5))
            .child(div().flex_1().child(Input::new(start_input)))
            .child(div().text_color(gpui::rgb(0x999999)).child(self.separator))
            .child(div().flex_1().child(Input::new(end_input)))
    }
}

/// RangeInput component module.
pub mod range_input {
    pub use super::{RangeInput, RangeInputState};
}

/// Editable select input state.
#[derive(Debug)]
pub struct SelectInputState {
    /// Current filter query.
    pub query: SharedString,
    /// Native editor for the filter query.
    pub input: Entity<InputState>,
    /// Selected option keys.
    pub selected: Vec<String>,
    /// Available options.
    pub options: Vec<SelectOption>,
    /// Multiple selection mode.
    pub multiple: bool,
    /// Popup visibility.
    pub open: bool,
    /// Highlighted option index for keyboard activation.
    pub highlighted: Option<usize>,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl SelectInputState {
    /// Creates select input state.
    pub fn new(cx: &mut App, options: Vec<SelectOption>) -> Entity<Self> {
        let input = InputState::new(cx, "");
        cx.new(|cx| {
            let subscription = cx.subscribe(
                &input,
                |state: &mut SelectInputState,
                 _input: Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<SelectInputState>| {
                    let InputEvent::Change(change) = event;
                    state.apply_query(change.current.clone(), cx);
                },
            );
            subscription.detach();
            let focus_handle = input.read(cx).focus_handle.clone();
            Self {
                query: SharedString::default(),
                input,
                selected: Vec::new(),
                options,
                multiple: false,
                open: false,
                highlighted: None,
                disabled: false,
                focus_handle,
            }
        })
    }

    /// Selects or toggles an enabled key.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if !self
            .options
            .iter()
            .any(|option| option.key == key && !option.disabled)
        {
            return false;
        }
        if self.multiple {
            if let Some(index) = self.selected.iter().position(|selected| selected == key) {
                self.selected.remove(index);
            } else {
                self.selected.push(key.to_owned());
            }
        } else {
            self.selected = vec![key.to_owned()];
            self.open = false;
        }
        self.highlighted = self.options.iter().position(|option| option.key == key);
        self.query = SharedString::default();
        if !self.input.read(cx).value.is_empty() {
            self.input.update(cx, |input, cx| {
                input.sync_value("", cx);
            });
        }
        cx.notify();
        true
    }

    /// Updates the filter query and opens the popup.
    pub fn set_query(
        &mut self,
        query: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let query = query.into();
        let change = self.apply_query(query.clone(), cx);
        if self.input.read(cx).value != query {
            self.input.update(cx, |input, cx| {
                input.sync_value(query, cx);
            });
        }
        change
    }

    fn apply_query(
        &mut self,
        query: SharedString,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let previous = self.query.clone();
        self.query = query;
        self.open = !self.disabled;
        let query = self.query.to_lowercase();
        self.highlighted = self.options.iter().position(|option| {
            !option.disabled && (query.is_empty() || option.label.to_lowercase().contains(&query))
        });
        if previous != self.query {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.query.clone(),
        }
    }

    /// Opens the popup and highlights the first matching option.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            let query = self.query.to_lowercase();
            self.highlighted = self.options.iter().position(|option| {
                !option.disabled
                    && (query.is_empty() || option.label.to_lowercase().contains(&query))
            });
        }
        cx.notify();
    }

    /// Closes the popup.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Moves the highlight through matching enabled options.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let query = self.query.to_lowercase();
        let matching = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                (!option.disabled
                    && (query.is_empty() || option.label.to_lowercase().contains(&query)))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return;
        }
        let was_open = self.open;
        self.open(cx);
        if !was_open {
            self.highlighted = if delta < 0 {
                matching.last().copied()
            } else {
                matching.first().copied()
            };
            cx.notify();
            return;
        }
        let current = self
            .highlighted
            .and_then(|index| matching.iter().position(|candidate| *candidate == index))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(matching.len() as i32) as usize;
        self.highlighted = Some(matching[next]);
        cx.notify();
    }

    /// Activates the highlighted option.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let index = self.highlighted?;
        let key = self.options.get(index)?.key.clone();
        self.select(&key, cx).then_some(key)
    }

    /// Clears all selected values and the current query.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.selected.clear();
        self.query = SharedString::default();
        if !self.input.read(cx).value.is_empty() {
            self.input.update(cx, |input, cx| {
                input.sync_value("", cx);
            });
        }
        self.highlighted = None;
        cx.notify();
    }
}

/// Searchable select field supporting single or multiple values.
#[derive(IntoElement)]
pub struct SelectInput {
    state: Entity<SelectInputState>,
    placeholder: SharedString,
    on_change: Option<Arc<dyn Fn(&[String], &mut Window, &mut App)>>,
}

impl SelectInput {
    /// Creates a select input.
    pub fn new(state: Entity<SelectInputState>) -> Self {
        Self {
            state,
            placeholder: "请选择".into(),
            on_change: None,
        }
    }

    /// Sets placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Registers selected-key changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[String], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for SelectInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let input_entity = self.state.read(cx).input.clone();
        let disabled = self.state.read(cx).disabled;
        let placeholder = self.placeholder.clone();
        let _ = input_entity.update(cx, |input, cx| {
            let changed = input.disabled != disabled || input.placeholder != placeholder;
            input.disabled = disabled;
            input.placeholder = placeholder.clone();
            if changed {
                cx.notify();
            }
        });
        let state = self.state.read(cx);
        let selected = state.selected.clone();
        let query = state.query.clone();
        let open = state.open;
        let highlighted = state.highlighted;
        let focus = state.focus_handle.clone();
        let labels = selected
            .iter()
            .filter_map(|key| {
                state
                    .options
                    .iter()
                    .find(|option| &option.key == key)
                    .map(|option| option.label.to_string())
            })
            .collect::<Vec<_>>();
        let options = state
            .options
            .iter()
            .filter(|option| {
                query.is_empty() || option.label.to_lowercase().contains(&query.to_lowercase())
            })
            .cloned()
            .collect::<Vec<_>>();
        let toggle_entity = self.state.clone();
        let select_entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let handler = self.on_change;
        let entries = options
            .into_iter()
            .map(|option| {
                let key = option.key.clone();
                let checked = selected.contains(&key);
                let entity = select_entity.clone();
                let handler = handler.clone();
                let is_highlighted = entity
                    .read(cx)
                    .options
                    .iter()
                    .position(|candidate| candidate.key == key)
                    == highlighted;
                div()
                    .id(SharedString::from(format!("select-input-{key}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .when(is_highlighted, |this| this.bg(gpui::rgb(0xe8f1ff)))
                    .when(option.disabled, |this| this.opacity(0.5))
                    .when(!option.disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xf3f3f3)))
                    })
                    .child(option.label)
                    .child(if checked { "✓" } else { "" })
                    .on_click(move |_, window, cx| {
                        let changed = entity.update(cx, |state, cx| state.select(&key, cx));
                        if changed && let Some(handler) = &handler {
                            let selected = entity.read(cx).selected.clone();
                            handler(&selected, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let display = (!labels.is_empty()).then(|| labels.join(", "));
        div()
            .id(entity_id("tdesign-select-input", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(200.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-select-input-trigger", &self.state))
                    .relative()
                    .w_full()
                    .child(Input::new(input_entity.clone()))
                    .children(display.map(|display| {
                        div()
                            .absolute()
                            .left_3()
                            .top_2()
                            .px_1()
                            .rounded_sm()
                            .bg(gpui::rgb(0xf3f3f3))
                            .text_sm()
                            .child(display)
                    }))
                    .child(
                        div()
                            .absolute()
                            .right_2()
                            .top_2()
                            .text_color(gpui::rgb(0x666666))
                            .child("⌄"),
                    )
                    .on_click(move |_, window, cx| {
                        if toggle_entity.read(cx).disabled {
                            return;
                        }
                        toggle_entity.read(cx).focus_handle.focus(window);
                        toggle_entity.update(cx, |state, cx| {
                            if state.open {
                                state.close(cx)
                            } else {
                                state.open(cx)
                            }
                        });
                    }),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .right_0()
                        .mt_1()
                        .id("tdesign-select-input-options")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(entries),
                )
            })
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "down" | "arrowdown" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                    }
                    "up" | "arrowup" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                    }
                    "enter" | "space" => {
                        cx.stop_propagation();
                        let selected = key_entity.update(cx, |state, cx| {
                            if state.open {
                                state.select_highlighted(cx)
                            } else {
                                state.open(cx);
                                None
                            }
                        });
                        if selected.is_some()
                            && let Some(handler) = &key_handler
                        {
                            let selected = key_entity.read(cx).selected.clone();
                            handler(&selected, window, cx);
                        }
                    }
                    "escape" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.close(cx));
                    }
                    _ => {}
                }
            })
    }
}

/// SelectInput component module.
pub mod select_input {
    pub use super::{SelectInput, SelectInputState};
}

/// One item that can move between transfer panels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferItem {
    /// Stable item key.
    pub key: String,
    /// Display label.
    pub label: SharedString,
    /// Disabled state.
    pub disabled: bool,
}

impl TransferItem {
    /// Creates a transfer item.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }
}

/// Dual-list transfer state.
#[derive(Debug)]
pub struct TransferState {
    /// All available items.
    pub items: Vec<TransferItem>,
    /// Keys currently in the target panel.
    pub target: BTreeSet<String>,
    /// Checked keys in either panel.
    pub checked: BTreeSet<String>,
    /// Item highlighted for keyboard interaction.
    pub highlighted: Option<String>,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl TransferState {
    /// Creates transfer state.
    pub fn new(cx: &mut App, items: Vec<TransferItem>) -> Entity<Self> {
        cx.new(|cx| Self {
            items,
            target: BTreeSet::new(),
            checked: BTreeSet::new(),
            highlighted: None,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Toggles one checked key.
    pub fn toggle(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.disabled
            || !self
                .items
                .iter()
                .any(|item| item.key == key && !item.disabled)
        {
            return false;
        }
        if self.checked.remove(key) {
            cx.notify();
            return true;
        }
        self.checked.insert(key.to_owned());
        self.highlighted = Some(key.to_owned());
        cx.notify();
        true
    }

    /// Moves checked source items to the target.
    pub fn move_to_target(&mut self, cx: &mut Context<Self>) -> bool {
        if self.disabled {
            return false;
        }
        let keys = self
            .checked
            .iter()
            .filter(|key| {
                !self.target.contains(*key)
                    && self
                        .items
                        .iter()
                        .any(|item| item.key.as_str() == key.as_str() && !item.disabled)
            })
            .cloned()
            .collect::<Vec<_>>();
        let changed = !keys.is_empty();
        for key in keys {
            self.target.insert(key.clone());
            self.checked.remove(&key);
        }
        if changed {
            cx.notify();
        }
        changed
    }

    /// Moves checked target items back to the source.
    pub fn move_to_source(&mut self, cx: &mut Context<Self>) -> bool {
        if self.disabled {
            return false;
        }
        let keys = self
            .checked
            .iter()
            .filter(|key| {
                self.target.contains(*key)
                    && self
                        .items
                        .iter()
                        .any(|item| item.key.as_str() == key.as_str() && !item.disabled)
            })
            .cloned()
            .collect::<Vec<_>>();
        let changed = !keys.is_empty();
        for key in keys {
            self.target.remove(&key);
            self.checked.remove(&key);
        }
        if changed {
            cx.notify();
        }
        changed
    }

    /// Moves the keyboard highlight through enabled items.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let keys = self
            .items
            .iter()
            .filter(|item| !item.disabled)
            .map(|item| item.key.clone())
            .collect::<Vec<_>>();
        if self.disabled || keys.is_empty() {
            return;
        }
        let current = self
            .highlighted
            .as_ref()
            .and_then(|key| keys.iter().position(|candidate| candidate == key));
        let next = match current {
            Some(current) => (current as i32 + delta).rem_euclid(keys.len() as i32) as usize,
            None if delta < 0 => keys.len() - 1,
            None => 0,
        };
        self.highlighted = Some(keys[next].clone());
        cx.notify();
    }

    /// Toggles the currently highlighted item.
    pub fn toggle_highlighted(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.highlighted.clone() else {
            return false;
        };
        self.toggle(&key, cx)
    }
}

/// Two-panel item transfer control.
#[derive(IntoElement)]
pub struct Transfer {
    state: Entity<TransferState>,
    on_change: Option<Arc<dyn Fn(&BTreeSet<String>, &mut Window, &mut App)>>,
}

impl Transfer {
    /// Creates a transfer control.
    pub fn new(state: Entity<TransferState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers target-key changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(&BTreeSet<String>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Transfer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let target = state.target.clone();
        let checked = state.checked.clone();
        let highlighted = state.highlighted.clone();
        let items = state.items.clone();
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let source_entity = self.state.clone();
        let target_entity = self.state.clone();
        let source_items = items
            .iter()
            .filter(|item| !target.contains(&item.key))
            .cloned()
            .map(|item| {
                transfer_row(
                    item,
                    checked.clone(),
                    highlighted.clone(),
                    source_entity.clone(),
                )
            })
            .collect::<Vec<_>>();
        let target_items = items
            .into_iter()
            .filter(|item| target.contains(&item.key))
            .map(|item| {
                transfer_row(
                    item,
                    checked.clone(),
                    highlighted.clone(),
                    target_entity.clone(),
                )
            })
            .collect::<Vec<_>>();
        let right_entity = self.state.clone();
        let left_entity = self.state.clone();
        let right_handler = self.on_change.clone();
        let left_handler = self.on_change;
        let key_entity = self.state.clone();
        div()
            .id(entity_id("tdesign-transfer", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_3()
            .when(disabled, |this| this.opacity(0.5))
            .child(transfer_panel("源列表", source_items))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .id(entity_id("transfer-right", &self.state))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(gpui::rgb(0x0052d9))
                            .text_color(gpui::white())
                            .child("›")
                            .on_click(move |_, window, cx| {
                                if right_entity.read(cx).disabled {
                                    return;
                                }
                                let changed =
                                    right_entity.update(cx, |state, cx| state.move_to_target(cx));
                                if changed && let Some(handler) = &right_handler {
                                    let target = right_entity.read(cx).target.clone();
                                    handler(&target, window, cx);
                                }
                            }),
                    )
                    .child(
                        div()
                            .id(entity_id("transfer-left", &self.state))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(gpui::rgb(0xdcdcdc))
                            .child("‹")
                            .on_click(move |_, window, cx| {
                                if left_entity.read(cx).disabled {
                                    return;
                                }
                                let changed =
                                    left_entity.update(cx, |state, cx| state.move_to_source(cx));
                                if changed && let Some(handler) = &left_handler {
                                    let target = left_entity.read(cx).target.clone();
                                    handler(&target, window, cx);
                                }
                            }),
                    ),
            )
            .child(transfer_panel("目标列表", target_items))
            .on_key_down(move |event, _, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "down" | "arrowdown" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                    }
                    "up" | "arrowup" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                    }
                    "space" => {
                        cx.stop_propagation();
                        key_entity.update(cx, |state, cx| state.toggle_highlighted(cx));
                    }
                    _ => {}
                }
            })
    }
}

fn transfer_row(
    item: TransferItem,
    checked: BTreeSet<String>,
    highlighted: Option<String>,
    entity: Entity<TransferState>,
) -> AnyElement {
    let key = item.key.clone();
    let selected = checked.contains(&key);
    let is_highlighted = highlighted.as_deref() == Some(key.as_str());
    div()
        .id(SharedString::from(format!("transfer-item-{key}")))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .when(item.disabled, |this| this.opacity(0.5))
        .when(is_highlighted, |this| this.bg(gpui::rgb(0xe8f1ff)))
        .child(if selected { "☑" } else { "☐" })
        .child(item.label)
        .on_click(move |_, _, cx| {
            if item.disabled || entity.read(cx).disabled {
                return;
            }
            entity.update(cx, |state, cx| state.toggle(&key, cx));
        })
        .into_any_element()
}

fn transfer_panel(title: &'static str, items: Vec<AnyElement>) -> impl IntoElement {
    div()
        .w(px(180.))
        .h(px(240.))
        .rounded_sm()
        .border_1()
        .border_color(gpui::rgb(0xe7e7e7))
        .child(
            div()
                .px_3()
                .py_2()
                .bg(gpui::rgb(0xf3f3f3))
                .border_b_1()
                .border_color(gpui::rgb(0xe7e7e7))
                .child(title),
        )
        .child(
            div()
                .id("tdesign-transfer-panel-items")
                .h_full()
                .overflow_y_scroll()
                .children(items),
        )
}

/// Transfer component module.
pub mod transfer {
    pub use super::{Transfer, TransferItem, TransferState};
}

/// Tree selector popup state.
#[derive(Debug)]
pub struct TreeSelectState {
    /// Root tree nodes.
    pub roots: Vec<TreeNode>,
    /// Selected key.
    pub selected: Option<String>,
    /// Key highlighted for keyboard activation.
    pub highlighted: Option<String>,
    /// Popup visibility.
    pub open: bool,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl TreeSelectState {
    /// Creates tree select state.
    pub fn new(cx: &mut App, roots: Vec<TreeNode>) -> Entity<Self> {
        cx.new(|cx| Self {
            roots,
            selected: None,
            highlighted: None,
            open: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Opens the tree popup and highlights the selected node or first enabled node.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            self.highlighted = self
                .selected
                .clone()
                .or_else(|| first_enabled_key(&self.roots));
        }
        cx.notify();
    }

    /// Closes the popup without changing the selected node.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Moves the keyboard highlight through enabled nodes in depth-first order.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let mut keys = Vec::new();
        collect_enabled_keys(&self.roots, &mut keys);
        if keys.is_empty() {
            return;
        }
        let was_open = self.open;
        self.open(cx);
        if !was_open {
            self.highlighted = if delta < 0 {
                keys.last().cloned()
            } else {
                keys.first().cloned()
            };
            cx.notify();
            return;
        }
        let current = self
            .highlighted
            .as_ref()
            .and_then(|key| keys.iter().position(|candidate| candidate == key))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(keys.len() as i32) as usize;
        self.highlighted = Some(keys[next].clone());
        cx.notify();
    }

    /// Selects the highlighted enabled node.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let key = self.highlighted.clone()?;
        self.select(&key, cx).then_some(key)
    }

    /// Selects an enabled node.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        fn contains(nodes: &[TreeNode], key: &str) -> bool {
            nodes.iter().any(|node| {
                if node.disabled {
                    return false;
                }
                if node.key == key {
                    true
                } else {
                    contains(&node.children, key)
                }
            })
        }
        if contains(&self.roots, key) {
            self.selected = Some(key.to_owned());
            self.highlighted = Some(key.to_owned());
            self.open = false;
            cx.notify();
            true
        } else {
            false
        }
    }
}

fn first_enabled_key(nodes: &[TreeNode]) -> Option<String> {
    let mut keys = Vec::new();
    collect_enabled_keys(nodes, &mut keys);
    keys.into_iter().next()
}

fn collect_enabled_keys(nodes: &[TreeNode], keys: &mut Vec<String>) {
    for node in nodes {
        if node.disabled {
            continue;
        }
        keys.push(node.key.clone());
        collect_enabled_keys(&node.children, keys);
    }
}

/// Compact tree-backed selector.
#[derive(IntoElement)]
pub struct TreeSelect {
    state: Entity<TreeSelectState>,
    placeholder: SharedString,
    on_change: Option<StringHandler>,
}

impl TreeSelect {
    /// Creates a tree selector.
    pub fn new(state: Entity<TreeSelectState>) -> Self {
        Self {
            state,
            placeholder: "请选择".into(),
            on_change: None,
        }
    }

    /// Sets placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Registers selection.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for TreeSelect {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        fn flatten(nodes: &[TreeNode], depth: usize, out: &mut Vec<(usize, TreeNode)>) {
            for node in nodes {
                out.push((depth, node.clone()));
                flatten(&node.children, depth + 1, out);
            }
        }
        fn label(nodes: &[TreeNode], key: &str) -> Option<SharedString> {
            for node in nodes {
                if node.key == key {
                    return Some(node.label.clone());
                }
                if let Some(label) = label(&node.children, key) {
                    return Some(label);
                }
            }
            None
        }
        let state = self.state.read(cx);
        let selected = state.selected.clone();
        let display = selected
            .as_deref()
            .and_then(|key| label(&state.roots, key))
            .unwrap_or(self.placeholder);
        let open = state.open;
        let highlighted = state.highlighted.clone();
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let mut flattened = Vec::new();
        flatten(&state.roots, 0, &mut flattened);
        let toggle_entity = self.state.clone();
        let select_entity = self.state.clone();
        let key_entity = self.state.clone();
        let handler = self.on_change;
        let nodes = flattened
            .into_iter()
            .map(|(depth, node)| {
                let key = node.key.clone();
                let is_selected = selected.as_deref() == Some(key.as_str());
                let is_highlighted = highlighted.as_deref() == Some(key.as_str());
                let entity = select_entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("tree-select-{key}")))
                    .pl(px(12. + depth as f32 * 16.))
                    .pr_3()
                    .py_2()
                    .when(is_selected, |this| {
                        this.bg(gpui::rgb(0xe8f1ff)).text_color(gpui::rgb(0x0052d9))
                    })
                    .when(is_highlighted && !is_selected, |this| {
                        this.bg(gpui::rgb(0xf3f7ff))
                    })
                    .when(node.disabled, |this| this.opacity(0.5))
                    .child(node.label)
                    .on_click(move |_, window, cx| {
                        let selected = entity.update(cx, |state, cx| state.select(&key, cx));
                        if selected {
                            if let Some(handler) = &handler {
                                handler(&key, window, cx);
                            }
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-tree-select", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(200.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-tree-select-trigger", &self.state))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_8()
                    .px_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .bg(gpui::white())
                    .child(display)
                    .child("⌄")
                    .on_click(move |_, window, cx| {
                        if toggle_entity.read(cx).disabled {
                            return;
                        }
                        toggle_entity.read(cx).focus_handle.focus(window);
                        let _ = toggle_entity.update(cx, |state, cx| {
                            if state.open {
                                state.close(cx);
                            } else {
                                state.open(cx);
                            }
                        });
                    }),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .right_0()
                        .mt_1()
                        .id("tdesign-tree-select-options")
                        .max_h(px(260.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(nodes),
                )
            })
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                key_entity.read(cx).focus_handle.focus(window);
                let key = event.keystroke.key.to_ascii_lowercase();
                match key.as_str() {
                    "down" | "arrowdown" => {
                        let _ = key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                    }
                    "up" | "arrowup" => {
                        let _ = key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                    }
                    "enter" | "space" => {
                        let selected = key_entity.update(cx, |state, cx| {
                            if state.open {
                                state.select_highlighted(cx)
                            } else {
                                state.open(cx);
                                None
                            }
                        });
                        if let Some(key) = selected
                            && let Some(handler) = &handler
                        {
                            handler(&key, window, cx);
                        }
                    }
                    "escape" => {
                        let _ = key_entity.update(cx, |state, cx| state.close(cx));
                    }
                    _ => {}
                }
            })
    }
}

/// TreeSelect component module.
pub mod tree_select {
    pub use super::{TreeSelect, TreeSelectState};
}
