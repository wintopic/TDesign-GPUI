//! Entity-backed input and selection components.

use crate::state::{MultilineLayout, multiline_position_for_offset};
use crate::{
    ComponentSize, HttpUploadBackend, InputState, Sizable, UploadBackend, UploadCancellation,
    UploadProgress, UploadRequest, ValueChange,
};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Timelike};
use futures::{
    FutureExt as _,
    channel::{mpsc, oneshot},
    select,
    stream::StreamExt as _,
};
use gpui::{
    App, AsyncApp, Bounds, ClickEvent, Context, Element, ElementId, ElementInputHandler, Entity,
    EventEmitter, FocusHandle, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    MouseButton, PaintQuad, ParentElement, PathPromptOptions, Pixels, RenderOnce, ShapedLine,
    SharedString, Style, Task, TextAlign, TextRun, UnderlineStyle, WeakEntity, Window, WrappedLine,
    div, fill, hsla, point, prelude::*, px, relative, rgba, size,
};
use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

struct TextFieldElement {
    state: Entity<InputState>,
}

struct TextFieldPrepaint {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

struct MultilineTextFieldElement {
    state: Entity<InputState>,
    rows: usize,
}

struct MultilineTextFieldPrepaint {
    lines: Vec<WrappedLine>,
    line_starts: Vec<usize>,
    line_height: Pixels,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = TextFieldPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        let content = state.value.clone();
        let selection = state.selection();
        let cursor = state.cursor_offset();
        let marked = state.marked_range();
        let style = window.text_style();
        let (display_text, color) = if content.is_empty() {
            (state.placeholder.clone(), hsla(0., 0., 0.5, 0.65))
        } else {
            (content, style.color)
        };
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = marked.filter(|range| range.end <= display_text.len()) {
            vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect::<Vec<_>>()
        } else {
            vec![run]
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let cursor_x = line.x_for_index(cursor.min(line.text.len()));
        let (selection_quad, cursor_quad) = if selection.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x, bounds.top()),
                        size(px(1.), bounds.bottom() - bounds.top()),
                    ),
                    gpui::rgb(0x0052d9),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selection.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selection.end),
                            bounds.bottom(),
                        ),
                    ),
                    rgba(0x0052d933),
                )),
                None,
            )
        };
        TextFieldPrepaint {
            line: Some(line),
            cursor: cursor_quad,
            selection: selection_quad,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.state.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().expect("prepaint line");
        let _ = line.paint(bounds.origin, window.line_height(), window, cx);
        if focus.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        let _ = self
            .state
            .update(cx, |state, _cx| state.set_layout(line, bounds));
    }
}

impl IntoElement for MultilineTextFieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MultilineTextFieldElement {
    type RequestLayoutState = ();
    type PrepaintState = MultilineTextFieldPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * self.rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self.state.read(cx);
        let content = state.value.clone();
        let selection = state.selection();
        let cursor = state.cursor_offset();
        let marked = state.marked_range();
        let style = window.text_style();
        let (display_text, color) = if content.is_empty() {
            (state.placeholder.clone(), hsla(0., 0., 0.5, 0.65))
        } else {
            (content, style.color)
        };
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = marked.filter(|range| range.end <= display_text.len()) {
            vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect::<Vec<_>>()
        } else {
            vec![run]
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let lines = window
            .text_system()
            .shape_text(
                display_text.clone(),
                font_size,
                &runs,
                Some(bounds.size.width),
                None,
            )
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let line_starts = multiline_line_starts(&display_text, &lines);
        let line_height = window.line_height();
        let layout = MultilineLayout {
            lines: lines.clone(),
            line_starts: line_starts.clone(),
            line_height,
        };
        let (cursor_quad, selection_quads) = if selection.is_empty() {
            let position = multiline_position_for_offset(&layout, cursor);
            (
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + position.x, bounds.top() + position.y),
                        size(px(1.), line_height),
                    ),
                    gpui::rgb(0x0052d9),
                )),
                Vec::new(),
            )
        } else {
            (None, multiline_selection_quads(&layout, selection, bounds))
        };
        MultilineTextFieldPrepaint {
            lines,
            line_starts,
            line_height,
            cursor: cursor_quad,
            selection: selection_quads,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.state.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.state.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let mut y = bounds.top();
        for line in &prepaint.lines {
            let _ = line.paint(
                point(bounds.left(), y),
                prepaint.line_height,
                TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
            y += line.size(prepaint.line_height).height;
        }
        if focus.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        let lines = std::mem::take(&mut prepaint.lines);
        let line_starts = std::mem::take(&mut prepaint.line_starts);
        let line_height = prepaint.line_height;
        let _ = self.state.update(cx, |state, _cx| {
            state.set_multiline_layout(lines, line_starts, line_height, bounds)
        });
    }
}

fn multiline_line_starts(text: &str, lines: &[WrappedLine]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut offset = 0;
    for line in lines {
        starts.push(offset);
        offset += line.text.len();
        if text.as_bytes().get(offset) == Some(&b'\n') {
            offset += 1;
        }
    }
    starts
}

fn multiline_selection_quads(
    layout: &MultilineLayout,
    selection: Range<usize>,
    bounds: Bounds<Pixels>,
) -> Vec<PaintQuad> {
    if selection.is_empty() {
        return Vec::new();
    }
    let start = selection.start.min(selection.end);
    let end = selection.start.max(selection.end);
    let start_position = multiline_position_for_offset(layout, start);
    let end_position = multiline_position_for_offset(layout, end);
    let left = bounds.left();
    let right = bounds.right();
    let top = bounds.top();
    let mut quads = Vec::new();
    if (start_position.y - end_position.y).abs() < px(0.01) {
        quads.push(fill(
            Bounds::from_corners(
                point(left + start_position.x, top + start_position.y),
                point(
                    left + end_position.x.max(start_position.x + px(1.)),
                    top + start_position.y + layout.line_height,
                ),
            ),
            rgba(0x0052d933),
        ));
    } else {
        quads.push(fill(
            Bounds::from_corners(
                point(left + start_position.x, top + start_position.y),
                point(right, top + start_position.y + layout.line_height),
            ),
            rgba(0x0052d933),
        ));
        quads.push(fill(
            Bounds::from_corners(
                point(left, top + end_position.y),
                point(
                    left + end_position.x.max(px(1.)),
                    top + end_position.y + layout.line_height,
                ),
            ),
            rgba(0x0052d933),
        ));
    }
    quads
}

/// Text field backed by [`InputState`].
#[derive(Clone, IntoElement)]
pub struct Input {
    state: Entity<InputState>,
    size: ComponentSize,
}
impl Input {
    /// Creates a controlled input.
    pub fn new(state: Entity<InputState>) -> Self {
        Self {
            state,
            size: ComponentSize::Medium,
        }
    }
    /// Returns the backing entity.
    pub fn state(&self) -> &Entity<InputState> {
        &self.state
    }
}
impl Sizable for Input {
    fn size(mut self, value: ComponentSize) -> Self {
        self.size = value;
        self
    }
}
impl RenderOnce for Input {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let state_entity = self.state.clone();
        let key_entity = self.state.clone();
        div()
            .id(entity_id("tdesign-input", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .h(self.size.height())
            .w_full()
            .px_3()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .bg(gpui::white())
            .overflow_hidden()
            .when(disabled, |this| this.opacity(0.5))
            .child(TextFieldElement {
                state: self.state.clone(),
            })
            .on_click(move |_, window, cx| {
                if !state_entity.read(cx).disabled {
                    state_entity.read(cx).focus_handle.focus(window);
                }
            })
            .on_key_down(move |event, window, cx| {
                let _ = key_entity.update(cx, |state, cx| state.handle_key_down(event, window, cx));
            })
    }
}
/// Input module.
pub mod input {
    pub use super::Input;
    pub use crate::InputState;
}

/// Multi-line text field sharing [`InputState`].
#[derive(Clone, Debug, IntoElement)]
pub struct Textarea {
    state: Entity<InputState>,
    rows: usize,
}
impl Textarea {
    /// Creates a textarea.
    pub fn new(state: Entity<InputState>) -> Self {
        Self { state, rows: 3 }
    }
    /// Sets the visible row count.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }
}
impl RenderOnce for Textarea {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = self.state.update(cx, |state, cx| {
            if !state.multiline {
                state.multiline = true;
                cx.notify();
            }
        });
        let state = self.state.read(cx);
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        div()
            .id(entity_id("tdesign-textarea", &self.state))
            .track_focus(&focus)
            .min_h(px(24. * self.rows as f32))
            .w_full()
            .p_3()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .bg(gpui::white())
            .overflow_hidden()
            .when(state.disabled, |this| this.opacity(0.5))
            .child(MultilineTextFieldElement {
                state: self.state.clone(),
                rows: self.rows,
            })
            .on_click(move |_, window, cx| {
                if !entity.read(cx).disabled {
                    entity.read(cx).focus_handle.focus(window);
                }
            })
            .on_key_down(move |event, window, cx| {
                let _ = key_entity.update(cx, |state, cx| state.handle_key_down(event, window, cx));
            })
    }
}
/// Textarea module.
pub mod textarea {
    pub use super::Textarea;
    pub use crate::InputState;
}

/// Numeric input state.
#[derive(Debug)]
pub struct NumberState {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl NumberState {
    /// Creates numeric input state.
    pub fn new(cx: &mut App, value: f64) -> Entity<Self> {
        cx.new(|cx| Self {
            value,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }
    /// Clamps, step-aligns, and assigns a value.
    pub fn set_value(&mut self, value: f64, cx: &mut Context<Self>) -> ValueChange<f64> {
        let previous = self.value;
        let mut value = if value.is_finite() { value } else { previous };
        value = value.clamp(self.min, self.max);
        if self.step.is_finite() && self.step > 0.0 && self.min.is_finite() {
            let steps = ((value - self.min) / self.step).round();
            value = (self.min + steps * self.step).clamp(self.min, self.max);
        }
        self.value = value;
        if (previous - self.value).abs() > f64::EPSILON {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.value,
        }
    }

    /// Increments by one configured step.
    pub fn increment(&mut self, cx: &mut Context<Self>) -> ValueChange<f64> {
        self.set_value(self.value + self.step.max(f64::EPSILON), cx)
    }

    /// Decrements by one configured step.
    pub fn decrement(&mut self, cx: &mut Context<Self>) -> ValueChange<f64> {
        self.set_value(self.value - self.step.max(f64::EPSILON), cx)
    }

    /// Formats the value without exposing floating-point representation noise.
    pub fn formatted_value(&self) -> String {
        let precision = decimal_places(self.step)
            .max(decimal_places(self.value.abs()))
            .min(12);
        let formatted = format!("{:.precision$}", self.value);
        if precision == 0 {
            formatted
        } else {
            formatted
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        }
    }
}

fn decimal_places(value: f64) -> usize {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }
    let text = format!("{value:.12}");
    text.trim_end_matches('0')
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len())
}

type NumberHandler = Arc<dyn Fn(ValueChange<f64>, &mut Window, &mut App)>;

/// Number field with increment and decrement controls.
#[derive(Clone, IntoElement)]
pub struct InputNumber {
    state: Entity<NumberState>,
    size: ComponentSize,
    on_change: Option<NumberHandler>,
}
impl InputNumber {
    /// Creates a numeric field.
    pub fn new(state: Entity<NumberState>) -> Self {
        Self {
            state,
            size: ComponentSize::Medium,
            on_change: None,
        }
    }

    /// Registers value changes from the step controls or keyboard.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<f64>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl Sizable for InputNumber {
    fn size(mut self, value: ComponentSize) -> Self {
        self.size = value;
        self
    }
}
impl RenderOnce for InputNumber {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let value: SharedString = state.formatted_value().into();
        let disabled = state.disabled;
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        let decrement_entity = self.state.clone();
        let increment_entity = self.state.clone();
        let decrement_handler = self.on_change.clone();
        let increment_handler = self.on_change.clone();
        let key_handler = self.on_change;
        let focus = state.focus_handle.clone();
        div()
            .id(entity_id("tdesign-input-number", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .h(self.size.height())
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id(
                        "tdesign-input-number-decrement",
                        &decrement_entity,
                    ))
                    .px_2()
                    .child("−")
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        let event = decrement_entity.update(cx, |state, cx| {
                            if !state.disabled {
                                Some(state.decrement(cx))
                            } else {
                                None
                            }
                        });
                        if let Some(event) = event
                            && event.changed()
                            && let Some(handler) = &decrement_handler
                        {
                            handler(event, window, cx);
                        }
                    }),
            )
            .child(div().px_3().child(value))
            .child(
                div()
                    .id(entity_id(
                        "tdesign-input-number-increment",
                        &increment_entity,
                    ))
                    .px_2()
                    .child("+")
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        let event = increment_entity.update(cx, |state, cx| {
                            if !state.disabled {
                                Some(state.increment(cx))
                            } else {
                                None
                            }
                        });
                        if let Some(event) = event
                            && event.changed()
                            && let Some(handler) = &increment_handler
                        {
                            handler(event, window, cx);
                        }
                    }),
            )
            .on_click(move |_, window, cx| {
                if !entity.read(cx).disabled {
                    entity.read(cx).focus_handle.focus(window);
                }
            })
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.to_ascii_lowercase();
                let change = key_entity.update(cx, |state, cx| {
                    if state.disabled {
                        return None;
                    }
                    match key.as_str() {
                        "up" | "arrowup" => Some(state.increment(cx)),
                        "down" | "arrowdown" => Some(state.decrement(cx)),
                        "home" => Some(state.set_value(state.min, cx)),
                        "end" => Some(state.set_value(state.max, cx)),
                        _ => None,
                    }
                });
                if let Some(change) = change
                    && change.changed()
                    && let Some(handler) = &key_handler
                {
                    handler(change, window, cx);
                }
            })
    }
}
/// InputNumber module.
pub mod input_number {
    pub use super::{InputNumber, NumberState};
}

/// Reusable state for Checkbox, Radio and Switch.
#[derive(Debug)]
pub struct ToggleState {
    pub checked: bool,
    pub indeterminate: bool,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl ToggleState {
    /// Creates toggle state.
    pub fn new(cx: &mut App, checked: bool) -> Entity<Self> {
        cx.new(|cx| Self {
            checked,
            indeterminate: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }
    /// Toggles the current value.
    pub fn toggle(&mut self, cx: &mut Context<Self>) -> ValueChange<bool> {
        let previous = self.checked;
        self.checked = !self.checked;
        self.indeterminate = false;
        cx.notify();
        ValueChange {
            previous,
            current: self.checked,
        }
    }

    /// Checks the control without toggling an already checked value.
    pub fn check(&mut self, cx: &mut Context<Self>) -> ValueChange<bool> {
        let previous = self.checked;
        let was_indeterminate = self.indeterminate;
        self.checked = true;
        self.indeterminate = false;
        if previous != self.checked || was_indeterminate {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.checked,
        }
    }
}

type ToggleHandler = Arc<dyn Fn(ValueChange<bool>, &mut Window, &mut App)>;

macro_rules! toggle_component {
    ($name:ident, $module:ident, $glyph_on:literal, $glyph_off:literal, $exclusive:literal) => {
        #[derive(IntoElement)]
        pub struct $name {
            state: Entity<ToggleState>,
            label: SharedString,
            on_change: Option<ToggleHandler>,
        }
        impl $name {
            pub fn new(state: Entity<ToggleState>, label: impl Into<SharedString>) -> Self {
                Self {
                    state,
                    label: label.into(),
                    on_change: None,
                }
            }
            pub fn on_change(
                mut self,
                handler: impl Fn(ValueChange<bool>, &mut Window, &mut App) + 'static,
            ) -> Self {
                self.on_change = Some(Arc::new(handler));
                self
            }
        }
        impl RenderOnce for $name {
            fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
                let state = self.state.read(cx);
                let glyph = if state.indeterminate {
                    "−"
                } else if state.checked {
                    $glyph_on
                } else {
                    $glyph_off
                };
                let disabled = state.disabled;
                let focus = state.focus_handle.clone();
                let entity = self.state.clone();
                let handler = self.on_change.clone();
                let click_entity = entity.clone();
                let key_entity = entity.clone();
                let click_handler = handler.clone();
                let key_handler = handler.clone();
                div()
                    .id(entity_id(stringify!($module), &entity))
                    .track_focus(&focus)
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(disabled, |this| this.opacity(0.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_5()
                            .h_5()
                            .rounded_sm()
                            .border_1()
                            .border_color(gpui::rgb(0x999999))
                            .when(state.checked, |this| {
                                this.bg(gpui::rgb(0x0052d9)).text_color(gpui::white())
                            })
                            .child(glyph),
                    )
                    .child(self.label)
                    .on_click(move |_, window, cx| {
                        if click_entity.read(cx).disabled {
                            return;
                        }
                        click_entity.read(cx).focus_handle.focus(window);
                        let event = click_entity.update(cx, |state, cx| {
                            if $exclusive {
                                state.check(cx)
                            } else {
                                state.toggle(cx)
                            }
                        });
                        if event.changed()
                            && let Some(handler) = &click_handler
                        {
                            handler(event, window, cx);
                        }
                    })
                    .on_key_down(move |event, window, cx| {
                        let key = event.keystroke.key.to_ascii_lowercase();
                        if matches!(key.as_str(), "space" | "enter") {
                            if key_entity.read(cx).disabled {
                                return;
                            }
                            cx.stop_propagation();
                            key_entity.read(cx).focus_handle.focus(window);
                            let change = key_entity.update(cx, |state, cx| {
                                if $exclusive {
                                    state.check(cx)
                                } else {
                                    state.toggle(cx)
                                }
                            });
                            if change.changed()
                                && let Some(handler) = &key_handler
                            {
                                handler(change, window, cx);
                            }
                        }
                    })
            }
        }
        pub mod $module {
            pub use super::{ToggleState, $name};
        }
    };
}
toggle_component!(Checkbox, checkbox, "✓", "", false);
toggle_component!(Radio, radio, "●", "", true);

/// Binary switch backed by [`ToggleState`].
#[derive(IntoElement)]
pub struct Switch {
    state: Entity<ToggleState>,
    on_change: Option<ToggleHandler>,
}
impl Switch {
    /// Creates a switch.
    pub fn new(state: Entity<ToggleState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }
    /// Registers a change callback.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<bool>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let checked = state.checked;
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        let click_handler = self.on_change.clone();
        let key_handler = self.on_change;
        div()
            .id(entity_id("tdesign-switch", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .w_10()
            .h_5()
            .p_0p5()
            .rounded_full()
            .bg(if checked {
                gpui::rgb(0x0052d9)
            } else {
                gpui::rgb(0xbfbfbf)
            })
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .w_4()
                    .h_4()
                    .rounded_full()
                    .bg(gpui::white())
                    .when(checked, |this| this.ml_auto()),
            )
            .on_click(move |_, window, cx| {
                if entity.read(cx).disabled {
                    return;
                }
                entity.read(cx).focus_handle.focus(window);
                let event = entity.update(cx, |state, cx| state.toggle(cx));
                if let Some(handler) = &click_handler {
                    handler(event, window, cx);
                }
            })
            .on_key_down(move |event, window, cx| {
                if !matches!(
                    event.keystroke.key.to_ascii_lowercase().as_str(),
                    "space" | "enter"
                ) || key_entity.read(cx).disabled
                {
                    return;
                }
                cx.stop_propagation();
                key_entity.read(cx).focus_handle.focus(window);
                let change = key_entity.update(cx, |state, cx| state.toggle(cx));
                if let Some(handler) = &key_handler {
                    handler(change, window, cx);
                }
            })
    }
}
/// Switch module.
pub mod switch {
    pub use super::{Switch, ToggleState};
}

/// Slider state with range and step constraints.
#[derive(Debug)]
pub struct SliderState {
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
    track_bounds: Option<Bounds<Pixels>>,
}
impl SliderState {
    /// Creates slider state.
    pub fn new(cx: &mut App, value: f32, min: f32, max: f32) -> Entity<Self> {
        let (min, max) = if min.is_finite() && max.is_finite() && min <= max {
            (min, max)
        } else if min.is_finite() {
            (min, min)
        } else if max.is_finite() {
            (max, max)
        } else {
            (0.0, 1.0)
        };
        cx.new(|cx| Self {
            value: value.clamp(min, max),
            min,
            max,
            step: 1.0,
            disabled: false,
            focus_handle: cx.focus_handle(),
            track_bounds: None,
        })
    }
    /// Assigns a constrained, step-aligned value.
    pub fn set_value(&mut self, value: f32, cx: &mut Context<Self>) -> ValueChange<f32> {
        let previous = self.value;
        let mut value = if value.is_finite() { value } else { previous };
        value = value.clamp(self.min, self.max);
        if self.step.is_finite() && self.step > 0.0 {
            let steps = ((value - self.min) / self.step).round();
            value = (self.min + steps * self.step).clamp(self.min, self.max);
        }
        self.value = value;
        if (previous - self.value).abs() > f32::EPSILON {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.value,
        }
    }

    /// Sets the value from a normalized `0.0..=1.0` track position.
    pub fn set_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) -> ValueChange<f32> {
        let ratio = if ratio.is_finite() {
            ratio.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.set_value(self.min + (self.max - self.min) * ratio, cx)
    }

    /// Increments by one configured step.
    pub fn increment(&mut self, cx: &mut Context<Self>) -> ValueChange<f32> {
        self.set_value(self.value + self.step.max(f32::EPSILON), cx)
    }

    /// Decrements by one configured step.
    pub fn decrement(&mut self, cx: &mut Context<Self>) -> ValueChange<f32> {
        self.set_value(self.value - self.step.max(f32::EPSILON), cx)
    }

    fn set_track_bounds(&mut self, bounds: Bounds<Pixels>) {
        self.track_bounds = Some(bounds);
    }

    fn ratio_for_position(&self, x: Pixels) -> Option<f32> {
        let bounds = self.track_bounds.as_ref()?;
        let width = bounds.size.width;
        if width <= Pixels::ZERO {
            return None;
        }
        Some(((x - bounds.origin.x) / width).clamp(0.0, 1.0))
    }
}

/// Horizontal value slider.
#[derive(Clone, IntoElement)]
pub struct Slider {
    state: Entity<SliderState>,
    on_change: Option<Arc<dyn Fn(ValueChange<f32>, &mut Window, &mut App)>>,
}
impl Slider {
    /// Creates a slider.
    pub fn new(state: Entity<SliderState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers a callback for value changes from pointer or keyboard input.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let ratio = if state.max > state.min {
            (state.value - state.min) / (state.max - state.min)
        } else {
            0.0
        };
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let bounds_entity = self.state.clone();
        let click_entity = self.state.clone();
        let drag_entity = self.state.clone();
        let key_entity = self.state.clone();
        let click_handler = self.on_change.clone();
        let drag_handler = self.on_change.clone();
        let key_handler = self.on_change;
        div()
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first().cloned() {
                    let _ = bounds_entity.update(cx, |state, _| state.set_track_bounds(bounds));
                }
            })
            .id(entity_id("tdesign-slider", &self.state))
            .track_focus(&focus)
            .relative()
            .w_40()
            .h_5()
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .absolute()
                    .top_2()
                    .left_0()
                    .w_full()
                    .h_1()
                    .rounded_full()
                    .bg(gpui::rgb(0xdcdcdc)),
            )
            .child(
                div()
                    .absolute()
                    .top_2()
                    .left_0()
                    .w(gpui::relative(ratio))
                    .h_1()
                    .rounded_full()
                    .bg(gpui::rgb(0x0052d9)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(gpui::relative(ratio))
                    .w_5()
                    .h_5()
                    .rounded_full()
                    .border_2()
                    .border_color(gpui::rgb(0x0052d9))
                    .bg(gpui::white()),
            )
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if click_entity.read(cx).disabled {
                    return;
                }
                click_entity.read(cx).focus_handle.focus(window);
                let change = click_entity.update(cx, |state, cx| {
                    state
                        .ratio_for_position(event.position.x)
                        .map(|ratio| state.set_ratio(ratio, cx))
                });
                if let Some(change) = change
                    && let Some(handler) = &click_handler
                {
                    handler(change, window, cx);
                }
            })
            .on_mouse_move(move |event, window, cx| {
                if !event.dragging() || drag_entity.read(cx).disabled {
                    return;
                }
                let change = drag_entity.update(cx, |state, cx| {
                    state
                        .ratio_for_position(event.position.x)
                        .map(|ratio| state.set_ratio(ratio, cx))
                });
                if let Some(change) = change
                    && let Some(handler) = &drag_handler
                {
                    handler(change, window, cx);
                }
            })
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                if !matches!(
                    key.as_str(),
                    "left"
                        | "arrowleft"
                        | "right"
                        | "arrowright"
                        | "up"
                        | "arrowup"
                        | "down"
                        | "arrowdown"
                        | "home"
                        | "end"
                ) {
                    return;
                }
                key_entity.read(cx).focus_handle.focus(window);
                let change = key_entity.update(cx, |state, cx| match key.as_str() {
                    "left" | "arrowleft" | "down" | "arrowdown" => state.decrement(cx),
                    "right" | "arrowright" | "up" | "arrowup" => state.increment(cx),
                    "home" => state.set_value(state.min, cx),
                    "end" => state.set_value(state.max, cx),
                    _ => unreachable!(),
                });
                if let Some(handler) = &key_handler {
                    handler(change, window, cx);
                }
            })
    }
}
/// Slider module.
pub mod slider {
    pub use super::{Slider, SliderState};
}

/// A key/label option used by selection components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectOption {
    pub key: String,
    pub label: SharedString,
    pub disabled: bool,
}
impl SelectOption {
    /// Creates an option.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }
}

/// State for Select, Cascader and TreeSelect adapters.
#[derive(Debug)]
pub struct SelectState {
    pub selected: Option<String>,
    pub options: Vec<SelectOption>,
    pub open: bool,
    /// Index of the option currently highlighted for keyboard activation.
    pub highlighted: Option<usize>,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl SelectState {
    /// Creates selection state.
    pub fn new(cx: &mut App, options: Vec<SelectOption>) -> Entity<Self> {
        cx.new(|cx| Self {
            selected: None,
            options,
            open: false,
            highlighted: None,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Opens the list and highlights the selected option or first enabled option.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            self.highlighted = self
                .selected
                .as_ref()
                .and_then(|key| self.options.iter().position(|option| &option.key == key))
                .or_else(|| self.options.iter().position(|option| !option.disabled));
        }
        cx.notify();
    }

    /// Closes the list without changing the selected value.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Moves the keyboard highlight, skipping disabled options and wrapping around.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.options.is_empty() {
            return;
        }
        let was_open = self.open;
        self.open(cx);
        let enabled = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| (!option.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return;
        }
        if !was_open {
            if delta < 0 {
                self.highlighted = enabled.last().copied();
                cx.notify();
            }
            return;
        }
        let current = self
            .highlighted
            .and_then(|index| enabled.iter().position(|candidate| *candidate == index))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(enabled.len() as i32) as usize;
        self.highlighted = Some(enabled[next]);
        cx.notify();
    }

    /// Activates the highlighted option, returning its stable key when changed.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let index = self.highlighted?;
        let key = self.options.get(index)?.key.clone();
        self.select(key, cx)
            .then_some(self.options[index].key.clone())
    }
    /// Selects a key if it exists and is enabled.
    pub fn select(&mut self, key: impl Into<String>, cx: &mut Context<Self>) -> bool {
        let key = key.into();
        if self
            .options
            .iter()
            .any(|option| option.key == key && !option.disabled)
        {
            self.selected = Some(key);
            self.open = false;
            self.highlighted = self.selected.as_ref().and_then(|selected| {
                self.options
                    .iter()
                    .position(|option| &option.key == selected)
            });
            cx.notify();
            true
        } else {
            false
        }
    }
}

/// Single-value selection control.
#[derive(Clone, IntoElement)]
pub struct Select {
    state: Entity<SelectState>,
    placeholder: Option<SharedString>,
    on_change: Option<Arc<dyn Fn(&str, &mut Window, &mut App)>>,
}
impl Select {
    /// Creates a select.
    pub fn new(state: Entity<SelectState>) -> Self {
        Self {
            state,
            placeholder: None,
            on_change: None,
        }
    }
    /// Sets placeholder text.
    pub fn placeholder(mut self, value: impl Into<SharedString>) -> Self {
        self.placeholder = Some(value.into());
        self
    }
    /// Registers enabled option selection from pointer or keyboard input.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Select {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let placeholder = self
            .placeholder
            .unwrap_or_else(|| crate::locale::text(cx, "select-placeholder"));
        let label = state
            .selected
            .as_ref()
            .and_then(|key| state.options.iter().find(|option| &option.key == key))
            .map(|option| option.label.clone())
            .unwrap_or(placeholder);
        let disabled = state.disabled;
        let open = state.open;
        let highlighted = state.highlighted;
        let options = state.options.clone();
        let focus = state.focus_handle.clone();
        let trigger_entity = self.state.clone();
        let key_entity = self.state.clone();
        let option_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let option_handler = self.on_change;
        let trigger = div()
            .id(entity_id("tdesign-select-trigger", &trigger_entity))
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .h_full()
            .child(label)
            .child("⌄")
            .on_click(move |_, window, cx| {
                if trigger_entity.read(cx).disabled {
                    return;
                }
                trigger_entity.read(cx).focus_handle.focus(window);
                let _ = trigger_entity.update(cx, |state, cx| {
                    if state.open {
                        state.close(cx);
                    } else {
                        state.open(cx);
                    }
                });
            });
        let options = options
            .into_iter()
            .enumerate()
            .map(|(index, option)| {
                let key = option.key.clone();
                let entity = option_entity.clone();
                let handler = option_handler.clone();
                div()
                    .id(SharedString::from(format!("tdesign-select-{key}")))
                    .w_full()
                    .px_3()
                    .py_2()
                    .when(highlighted == Some(index), |this| {
                        this.bg(gpui::rgb(0xe8f1ff)).text_color(gpui::rgb(0x0052d9))
                    })
                    .when(option.disabled, |this| this.opacity(0.5))
                    .when(!option.disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xf3f3f3)))
                    })
                    .child(option.label)
                    .on_click(move |_, window, cx| {
                        let selected = entity.update(cx, |state, cx| state.select(key.clone(), cx));
                        if selected && let Some(handler) = &handler {
                            handler(&key, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-select", &self.state))
            .track_focus(&focus)
            .relative()
            .h_8()
            .min_w(px(160.))
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xdcdcdc))
            .when(disabled, |this| this.opacity(0.5))
            .child(trigger)
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .right_0()
                        .mt_1()
                        .py_1()
                        .id("tdesign-select-options")
                        .max_h(px(240.))
                        .overflow_y_scroll()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(options),
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
/// Select module.
pub mod select {
    pub use super::{Select, SelectOption, SelectState};
}

/// State for calendar-based date controls.
#[derive(Debug)]
pub struct DatePickerState {
    pub value: Option<NaiveDate>,
    pub min: Option<NaiveDate>,
    pub max: Option<NaiveDate>,
    pub open: bool,
    pub view_month: NaiveDate,
    pub highlighted: Option<NaiveDate>,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl DatePickerState {
    /// Creates date state.
    pub fn new(cx: &mut App, value: Option<NaiveDate>) -> Entity<Self> {
        let today = chrono::Local::now().date_naive();
        let initial = value.unwrap_or(today);
        cx.new(|cx| Self {
            value,
            min: None,
            max: None,
            open: false,
            view_month: initial.with_day(1).expect("valid first day"),
            highlighted: Some(initial),
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Opens the calendar popup and initializes its highlighted date.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        if self.highlighted.is_none() {
            self.highlighted = self.value.or(Some(chrono::Local::now().date_naive()));
        }
        if let Some(date) = self.highlighted {
            self.view_month = date.with_day(1).expect("valid first day");
        }
        cx.notify();
    }

    /// Closes the calendar popup without changing the value.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Returns whether a date cannot be selected.
    pub fn is_date_disabled(&self, date: NaiveDate) -> bool {
        self.disabled
            || self.min.is_some_and(|min| date < min)
            || self.max.is_some_and(|max| date > max)
    }

    /// Assigns an enabled date and returns its typed value change.
    pub fn set_value(
        &mut self,
        date: Option<NaiveDate>,
        cx: &mut Context<Self>,
    ) -> ValueChange<Option<NaiveDate>> {
        let previous = self.value;
        if date.is_some_and(|date| self.is_date_disabled(date)) {
            return ValueChange {
                previous,
                current: previous,
            };
        }
        let next = date;
        self.value = next;
        if let Some(date) = next {
            self.highlighted = Some(date);
            self.view_month = date.with_day(1).expect("valid first day");
        }
        self.open = false;
        if previous != self.value {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.value,
        }
    }

    /// Selects an enabled date, returning whether the request was accepted.
    pub fn select(&mut self, date: NaiveDate, cx: &mut Context<Self>) -> bool {
        self.select_change(date, cx).is_some()
    }

    /// Selects an enabled date and returns the typed change event.
    pub fn select_change(
        &mut self,
        date: NaiveDate,
        cx: &mut Context<Self>,
    ) -> Option<ValueChange<Option<NaiveDate>>> {
        if self.is_date_disabled(date) {
            return None;
        }
        Some(self.set_value(Some(date), cx))
    }

    /// Moves the visible month by a signed number of months.
    pub fn shift_month(&mut self, amount: i32, cx: &mut Context<Self>) {
        let current = i64::from(self.view_month.year()) * 12 + i64::from(self.view_month.month0());
        let target = current.saturating_add(i64::from(amount));
        let year = target.div_euclid(12);
        let month = target.rem_euclid(12) as u32 + 1;
        let Ok(year) = i32::try_from(year) else {
            return;
        };
        if let Some(month) = NaiveDate::from_ymd_opt(year, month, 1)
            && month != self.view_month
        {
            self.view_month = month;
            cx.notify();
        }
    }

    /// Moves the highlighted date by a number of days, skipping disabled dates.
    pub fn move_highlight(&mut self, days: i32, cx: &mut Context<Self>) {
        if days == 0 || self.disabled {
            return;
        }
        let mut date = self
            .highlighted
            .or(self.value)
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let direction = i64::from(days.signum());
        let Some(mut candidate) = date.checked_add_signed(Duration::days(i64::from(days))) else {
            return;
        };
        if let Some(min) = self.min
            && candidate < min
        {
            candidate = min;
        }
        if let Some(max) = self.max
            && candidate > max
        {
            candidate = max;
        }
        while self.is_date_disabled(candidate) {
            let Some(next) = candidate.checked_add_signed(Duration::days(direction)) else {
                return;
            };
            if self.min.is_some_and(|min| next < min) || self.max.is_some_and(|max| next > max) {
                return;
            }
            candidate = next;
        }
        if candidate != date {
            date = candidate;
            self.highlighted = Some(date);
            if let Some(month) = date.with_day(1) {
                self.view_month = month;
            }
            cx.notify();
        }
    }

    /// Selects the currently highlighted date.
    pub fn select_highlighted(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<ValueChange<Option<NaiveDate>>> {
        let date = self.highlighted?;
        if self.is_date_disabled(date) {
            return None;
        }
        Some(self.set_value(Some(date), cx))
    }
}
/// Native date picker value field.
#[derive(Clone, IntoElement)]
pub struct DatePicker {
    state: Entity<DatePickerState>,
    format: SharedString,
    on_change: Option<Arc<dyn Fn(ValueChange<Option<NaiveDate>>, &mut Window, &mut App)>>,
}
impl DatePicker {
    /// Creates a date picker.
    pub fn new(state: Entity<DatePickerState>) -> Self {
        Self {
            state,
            format: "%Y-%m-%d".into(),
            on_change: None,
        }
    }
    /// Sets chrono formatting syntax.
    pub fn format(mut self, format: impl Into<SharedString>) -> Self {
        self.format = format.into();
        self
    }

    /// Registers date changes from the popup or keyboard.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<Option<NaiveDate>>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for DatePicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let value: SharedString = state
            .value
            .map(|date| date.format(&self.format).to_string())
            .map(SharedString::from)
            .unwrap_or_else(|| crate::locale::text(cx, "select-date"));
        let open = state.open;
        let disabled = state.disabled;
        let month = state.view_month;
        let selected = state.value;
        let highlighted = state.highlighted;
        let focus = state.focus_handle.clone();
        let toggle_entity = self.state.clone();
        let key_entity = self.state.clone();
        let previous = self.state.clone();
        let next = self.state.clone();
        let select_entity = self.state.clone();
        let handler = self.on_change;
        let offset = month.weekday().num_days_from_sunday() as i64;
        let start = month - Duration::days(offset);
        let mut cells = Vec::with_capacity(42);
        for index in 0..42 {
            let date = start + Duration::days(index);
            let in_month = date.month() == month.month();
            let is_selected = selected == Some(date);
            let is_highlighted = highlighted == Some(date);
            let date_disabled = state.is_date_disabled(date);
            let entity = select_entity.clone();
            let handler = handler.clone();
            cells.push(
                div()
                    .id(SharedString::from(format!("date-picker-{date}")))
                    .flex()
                    .items_center()
                    .justify_center()
                    .h_8()
                    .rounded_sm()
                    .when(!in_month, |this| this.text_color(gpui::rgb(0xbdbdbd)))
                    .when(is_highlighted && !is_selected, |this| {
                        this.bg(gpui::rgb(0xf3f7ff))
                    })
                    .when(is_selected, |this| {
                        this.bg(gpui::rgb(0x0052d9)).text_color(gpui::white())
                    })
                    .when(date_disabled, |this| this.opacity(0.45))
                    .when(!date_disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xe8f1ff)))
                    })
                    .child(date.day().to_string())
                    .on_click(move |_, window, cx| {
                        let change = entity.update(cx, |state, cx| state.select_change(date, cx));
                        if let Some(change) = change
                            && let Some(handler) = &handler
                        {
                            handler(change, window, cx);
                        }
                    })
                    .into_any_element(),
            );
        }
        let month_title: SharedString = format!("{}年{}月", month.year(), month.month()).into();
        let mut picker = div()
            .id(entity_id("tdesign-date-picker", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(180.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-date-picker-trigger", &self.state))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_8()
                    .px_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .bg(gpui::white())
                    .child(value)
                    .child("▣")
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
            );
        if open {
            picker = picker.child(
                div()
                    .absolute()
                    .top_full()
                    .left_0()
                    .mt_1()
                    .w(px(320.))
                    .p_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .bg(gpui::white())
                    .shadow_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pb_2()
                            .child(
                                div()
                                    .id(entity_id("date-picker-previous", &self.state))
                                    .px_2()
                                    .child("‹")
                                    .on_click(move |_, _, cx| {
                                        previous.update(cx, |state, cx| state.shift_month(-1, cx));
                                    }),
                            )
                            .child(month_title)
                            .child(
                                div()
                                    .id(entity_id("date-picker-next", &self.state))
                                    .px_2()
                                    .child("›")
                                    .on_click(move |_, _, cx| {
                                        next.update(cx, |state, cx| state.shift_month(1, cx));
                                    }),
                            ),
                    )
                    .child(
                        div().flex().children(
                            ["日", "一", "二", "三", "四", "五", "六"]
                                .map(|day| div().flex_1().text_center().text_sm().child(day)),
                        ),
                    )
                    .child(div().grid().grid_cols(7).gap_1().children(cells)),
            );
        }
        picker = picker.on_key_down(move |event, window, cx| {
            if key_entity.read(cx).disabled {
                return;
            }
            let key = event.keystroke.key.to_ascii_lowercase();
            let result = key_entity.update(cx, |state, cx| match key.as_str() {
                "left" | "arrowleft" => {
                    state.move_highlight(-1, cx);
                    None
                }
                "right" | "arrowright" => {
                    state.move_highlight(1, cx);
                    None
                }
                "up" | "arrowup" => {
                    state.move_highlight(-7, cx);
                    None
                }
                "down" | "arrowdown" => {
                    state.move_highlight(7, cx);
                    None
                }
                "pageup" => {
                    state.shift_month(-1, cx);
                    None
                }
                "pagedown" => {
                    state.shift_month(1, cx);
                    None
                }
                "enter" | "space" if state.open => state.select_highlighted(cx),
                "enter" | "space" => {
                    state.open(cx);
                    None
                }
                "escape" => {
                    state.close(cx);
                    None
                }
                _ => None,
            });
            if let Some(handler) = &handler
                && let Some(event) = result
            {
                handler(event, window, cx);
            }
        });
        picker
    }
}
/// DatePicker module.
pub mod date_picker {
    pub use super::{DatePicker, DatePickerState};
}

/// State for time controls.
#[derive(Debug)]
pub struct TimePickerState {
    pub value: Option<NaiveTime>,
    pub min: Option<NaiveTime>,
    pub max: Option<NaiveTime>,
    pub open: bool,
    pub step: u32,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl TimePickerState {
    /// Creates time state.
    pub fn new(cx: &mut App, value: Option<NaiveTime>) -> Entity<Self> {
        cx.new(|cx| Self {
            value,
            min: None,
            max: None,
            open: false,
            step: 60,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Opens the time popup.
    pub fn open(&mut self, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        self.open = true;
        cx.notify();
    }

    /// Closes the time popup.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            cx.notify();
        }
    }

    /// Returns whether a time is outside the configured range.
    pub fn is_time_disabled(&self, time: NaiveTime) -> bool {
        self.disabled
            || self.min.is_some_and(|min| time < min)
            || self.max.is_some_and(|max| time > max)
    }

    /// Assigns a time and returns its typed value change.
    pub fn set_value(
        &mut self,
        time: Option<NaiveTime>,
        cx: &mut Context<Self>,
    ) -> ValueChange<Option<NaiveTime>> {
        let previous = self.value;
        self.value = match time {
            Some(time) if self.is_time_disabled(time) => previous,
            other => other,
        };
        if previous != self.value {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.value,
        }
    }

    /// Adjusts the current time by signed seconds, clamping to min/max.
    pub fn adjust_seconds(
        &mut self,
        seconds: i64,
        cx: &mut Context<Self>,
    ) -> ValueChange<Option<NaiveTime>> {
        let current = self.value.unwrap_or_default();
        let (wrapped, day_delta) = current.overflowing_add_signed(Duration::seconds(seconds));
        let candidate = if day_delta > 0 && seconds > 0 {
            self.max.unwrap_or(wrapped)
        } else if day_delta < 0 && seconds < 0 {
            self.min.unwrap_or(wrapped)
        } else if seconds < 0 {
            self.min.map_or(wrapped, |min| wrapped.max(min))
        } else {
            let candidate = self.min.map_or(wrapped, |min| wrapped.max(min));
            self.max.map_or(candidate, |max| candidate.min(max))
        };
        self.set_value(Some(candidate), cx)
    }

    /// Adjusts by one configured step.
    pub fn increment(&mut self, cx: &mut Context<Self>) -> ValueChange<Option<NaiveTime>> {
        self.adjust_seconds(self.step.max(1) as i64, cx)
    }

    /// Adjusts by one configured step in the negative direction.
    pub fn decrement(&mut self, cx: &mut Context<Self>) -> ValueChange<Option<NaiveTime>> {
        self.adjust_seconds(-(self.step.max(1) as i64), cx)
    }
}
/// Native time picker value field.
#[derive(Clone, IntoElement)]
pub struct TimePicker {
    state: Entity<TimePickerState>,
    format: SharedString,
    on_change: Option<Arc<dyn Fn(ValueChange<Option<NaiveTime>>, &mut Window, &mut App)>>,
}
impl TimePicker {
    /// Creates a time picker.
    pub fn new(state: Entity<TimePickerState>) -> Self {
        Self {
            state,
            format: "%H:%M:%S".into(),
            on_change: None,
        }
    }

    /// Sets chrono formatting syntax.
    pub fn format(mut self, format: impl Into<SharedString>) -> Self {
        self.format = format.into();
        self
    }

    /// Registers time changes from the popup or keyboard.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<Option<NaiveTime>>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for TimePicker {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let value: SharedString = state
            .value
            .map(|time| time.format(&self.format).to_string())
            .map(SharedString::from)
            .unwrap_or_else(|| crate::locale::text(cx, "select-time"));
        let open = state.open;
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let toggle_entity = self.state.clone();
        let key_entity = self.state.clone();
        let up_entity = self.state.clone();
        let down_entity = self.state.clone();
        let handler = self.on_change;
        let mut picker = div()
            .id(entity_id("tdesign-time-picker", &self.state))
            .track_focus(&focus)
            .relative()
            .min_w(px(160.))
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id(entity_id("tdesign-time-picker-trigger", &self.state))
                    .flex()
                    .items_center()
                    .justify_between()
                    .h_8()
                    .px_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .bg(gpui::white())
                    .child(value)
                    .child("◷")
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
            );
        if open {
            let time = state
                .value
                .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).expect("valid midnight"));
            let up_handler = handler.clone();
            let down_handler = handler.clone();
            picker = picker.child(
                div()
                    .absolute()
                    .top_full()
                    .left_0()
                    .mt_1()
                    .w(px(220.))
                    .p_3()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .bg(gpui::white())
                    .shadow_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(format!("{:02}", time.hour()))
                            .child(":")
                            .child(format!("{:02}", time.minute()))
                            .child(":")
                            .child(format!("{:02}", time.second())),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_center()
                            .gap_2()
                            .pt_2()
                            .child(
                                div()
                                    .id(entity_id("time-picker-up", &up_entity))
                                    .px_3()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(gpui::rgb(0xf3f3f3))
                                    .child("＋")
                                    .on_click(move |_, window, cx| {
                                        let event =
                                            up_entity.update(cx, |state, cx| state.increment(cx));
                                        if let Some(handler) = &up_handler {
                                            handler(event, window, cx);
                                        }
                                    }),
                            )
                            .child(
                                div()
                                    .id(entity_id("time-picker-down", &down_entity))
                                    .px_3()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(gpui::rgb(0xf3f3f3))
                                    .child("－")
                                    .on_click(move |_, window, cx| {
                                        let event =
                                            down_entity.update(cx, |state, cx| state.decrement(cx));
                                        if let Some(handler) = &down_handler {
                                            handler(event, window, cx);
                                        }
                                    }),
                            ),
                    ),
            );
        }
        picker = picker.on_key_down(move |event, window, cx| {
            if key_entity.read(cx).disabled {
                return;
            }
            let key = event.keystroke.key.to_ascii_lowercase();
            if matches!(key.as_str(), "space" | "enter") && !key_entity.read(cx).open {
                key_entity.read(cx).focus_handle.focus(window);
            }
            let change = key_entity.update(cx, |state, cx| match key.as_str() {
                "up" | "arrowup" | "right" | "arrowright" => Some(state.increment(cx)),
                "down" | "arrowdown" | "left" | "arrowleft" => Some(state.decrement(cx)),
                "enter" | "space" if state.open => {
                    state.close(cx);
                    None
                }
                "enter" | "space" => {
                    state.open(cx);
                    None
                }
                "escape" => {
                    state.close(cx);
                    None
                }
                _ => None,
            });
            if let Some(handler) = &handler
                && let Some(change) = change
            {
                handler(change, window, cx);
            }
        });
        picker
    }
}
/// TimePicker module.
pub mod time_picker {
    pub use super::{TimePicker, TimePickerState};
}

/// Upload status for one selected path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadStatus {
    Ready,
    Uploading,
    Success(SharedString),
    Failed(SharedString),
    Canceled,
}

/// Events emitted by [`UploadState`] as files move through the upload lifecycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadEvent {
    /// One or more paths were accepted by the file selector.
    FilesSelected { paths: Vec<PathBuf> },
    /// An upload attempt started.
    Started { path: PathBuf },
    /// An upload backend reported byte progress.
    Progress {
        path: PathBuf,
        uploaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    /// An upload completed successfully.
    Succeeded { path: PathBuf, url: SharedString },
    /// An upload failed.
    Failed { path: PathBuf, error: SharedString },
    /// An upload was cooperatively cancelled.
    Canceled { path: PathBuf },
    /// A path was removed from the queue.
    Removed { path: PathBuf },
    /// A failed/cancelled path was reset for another attempt.
    Retried { path: PathBuf },
}

/// Opaque handle for one path-specific upload generation.
///
/// Passing this handle back to the attempt-aware state methods prevents a
/// delayed task from mutating a newer retry of the same local path.
#[derive(Clone, Debug)]
pub struct UploadAttempt {
    path: PathBuf,
    generation: u64,
    cancellation: UploadCancellation,
}
impl UploadAttempt {
    /// Returns the local path associated with this attempt.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the cooperative cancellation handle for the backend request.
    pub fn cancellation(&self) -> UploadCancellation {
        self.cancellation.clone()
    }

    /// Returns whether cancellation was requested for this attempt.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

/// State for native file selection and injected upload transport.
#[derive(Debug, Default)]
pub struct UploadState {
    pub files: Vec<(PathBuf, UploadStatus)>,
    pub disabled: bool,
    progress: HashMap<PathBuf, UploadProgress>,
    cancellations: HashMap<PathBuf, UploadCancellation>,
    attempts: HashMap<PathBuf, u64>,
    next_attempt: u64,
}
impl EventEmitter<UploadEvent> for UploadState {}
impl UploadState {
    /// Creates upload state.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self::default())
    }

    /// Adds paths to the queue, de-duplicating entries and honoring `multiple`.
    pub fn add_files(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        multiple: bool,
        cx: &mut Context<Self>,
    ) -> Vec<PathBuf> {
        if self.disabled {
            return Vec::new();
        }
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        if !multiple && !paths.is_empty() {
            paths.truncate(1);
            let previous = self
                .files
                .iter()
                .map(|(path, _)| path.clone())
                .collect::<Vec<_>>();
            for path in previous {
                self.remove_file(&path, cx);
            }
        }

        let mut accepted = Vec::new();
        for path in paths {
            if self.files.iter().any(|(existing, _)| existing == &path) {
                continue;
            }
            self.progress
                .insert(path.clone(), UploadProgress::default());
            self.files.push((path.clone(), UploadStatus::Ready));
            accepted.push(path);
            if !multiple {
                break;
            }
        }
        if !accepted.is_empty() {
            cx.emit(UploadEvent::FilesSelected {
                paths: accepted.clone(),
            });
            cx.notify();
        }
        accepted
    }

    /// Returns the current status for a path.
    pub fn status(&self, path: &Path) -> Option<&UploadStatus> {
        self.files
            .iter()
            .find_map(|(current, status)| (current == path).then_some(status))
    }

    /// Returns the latest byte progress for a path.
    pub fn progress(&self, path: &Path) -> Option<UploadProgress> {
        self.progress.get(path).copied()
    }

    /// Marks a queued path as uploading and returns its cancellation handle.
    ///
    /// This compatibility method operates on the current path generation. New
    /// orchestration code should prefer [`Self::begin_attempt`] and the
    /// attempt-aware progress/completion methods.
    pub fn begin_upload(
        &mut self,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Option<UploadCancellation> {
        self.begin_attempt(path, cx)
            .map(|attempt| attempt.cancellation)
    }

    /// Marks a queued path as uploading and returns a generation-safe handle.
    pub fn begin_attempt(&mut self, path: &Path, cx: &mut Context<Self>) -> Option<UploadAttempt> {
        if self.disabled {
            return None;
        }
        let entry = self.files.iter_mut().find(|(current, _)| current == path)?;
        if matches!(entry.1, UploadStatus::Uploading) {
            return None;
        }
        let cancellation = UploadCancellation::new();
        self.next_attempt = self.next_attempt.wrapping_add(1).max(1);
        let attempt = self.next_attempt;
        entry.1 = UploadStatus::Uploading;
        self.progress.insert(
            path.to_path_buf(),
            UploadProgress::new(
                0,
                std::fs::metadata(path).ok().map(|metadata| metadata.len()),
            ),
        );
        self.cancellations
            .insert(path.to_path_buf(), cancellation.clone());
        self.attempts.insert(path.to_path_buf(), attempt);
        cx.emit(UploadEvent::Started {
            path: path.to_path_buf(),
        });
        cx.notify();
        Some(UploadAttempt {
            path: path.to_path_buf(),
            generation: attempt,
            cancellation,
        })
    }

    /// Applies a progress sample to the current active generation for a path.
    pub fn set_progress(&mut self, path: &Path, progress: UploadProgress, cx: &mut Context<Self>) {
        let Some(generation) = self.attempts.get(path).copied() else {
            return;
        };
        self.set_progress_for_generation(path, generation, progress, cx);
    }

    /// Applies progress only if `attempt` is still the current path generation.
    pub fn set_attempt_progress(
        &mut self,
        attempt: &UploadAttempt,
        progress: UploadProgress,
        cx: &mut Context<Self>,
    ) {
        self.set_progress_for_generation(&attempt.path, attempt.generation, progress, cx);
    }

    fn set_progress_for_generation(
        &mut self,
        path: &Path,
        generation: u64,
        progress: UploadProgress,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_attempt(path, generation) {
            return;
        }
        self.progress.insert(path.to_path_buf(), progress);
        cx.emit(UploadEvent::Progress {
            path: path.to_path_buf(),
            uploaded_bytes: progress.uploaded_bytes,
            total_bytes: progress.total_bytes,
        });
        cx.notify();
    }

    /// Marks the current active generation for a path as successful.
    pub fn finish_success(
        &mut self,
        path: &Path,
        url: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(generation) = self.attempts.get(path).copied() else {
            return;
        };
        self.finish_success_for_generation(path, generation, url.into(), cx);
    }

    /// Marks an upload successful only if `attempt` is still current.
    pub fn finish_attempt_success(
        &mut self,
        attempt: &UploadAttempt,
        url: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.finish_success_for_generation(&attempt.path, attempt.generation, url.into(), cx);
    }

    fn finish_success_for_generation(
        &mut self,
        path: &Path,
        generation: u64,
        url: SharedString,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_attempt(path, generation) {
            return;
        }
        let Some((_, status)) = self.files.iter_mut().find(|(current, _)| current == path) else {
            return;
        };
        if let Some(cancellation) = self.cancellations.get(path) {
            if cancellation.is_cancelled() {
                self.cancel_generation(path, generation, cx);
                return;
            }
        }
        *status = UploadStatus::Success(url.clone());
        if let Some(progress) = self.progress.get_mut(path) {
            progress.uploaded_bytes = progress.total_bytes.unwrap_or(progress.uploaded_bytes);
        }
        self.cancellations.remove(path);
        self.attempts.remove(path);
        cx.emit(UploadEvent::Succeeded {
            path: path.to_path_buf(),
            url,
        });
        cx.notify();
    }

    /// Marks the current active generation for a path as failed.
    pub fn finish_failure(
        &mut self,
        path: &Path,
        error: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(generation) = self.attempts.get(path).copied() else {
            return;
        };
        self.finish_failure_for_generation(path, generation, error.into(), cx);
    }

    /// Marks an upload failed only if `attempt` is still current.
    pub fn finish_attempt_failure(
        &mut self,
        attempt: &UploadAttempt,
        error: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.finish_failure_for_generation(&attempt.path, attempt.generation, error.into(), cx);
    }

    fn finish_failure_for_generation(
        &mut self,
        path: &Path,
        generation: u64,
        error: SharedString,
        cx: &mut Context<Self>,
    ) {
        if !self.is_current_attempt(path, generation) {
            return;
        }
        if self
            .cancellations
            .get(path)
            .is_some_and(UploadCancellation::is_cancelled)
        {
            self.cancel_generation(path, generation, cx);
            return;
        }
        let Some((_, status)) = self.files.iter_mut().find(|(current, _)| current == path) else {
            return;
        };
        *status = UploadStatus::Failed(error.clone());
        self.cancellations.remove(path);
        self.attempts.remove(path);
        cx.emit(UploadEvent::Failed {
            path: path.to_path_buf(),
            error,
        });
        cx.notify();
    }

    /// Requests cancellation of an active upload.
    pub fn cancel_file(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        let Some(attempt) = self.attempts.get(path).copied() else {
            let Some((_, status)) = self.files.iter_mut().find(|(current, _)| current == path)
            else {
                return false;
            };
            if matches!(status, UploadStatus::Canceled) {
                return true;
            }
            *status = UploadStatus::Canceled;
            cx.emit(UploadEvent::Canceled {
                path: path.to_path_buf(),
            });
            cx.notify();
            return true;
        };
        self.cancel_generation(path, attempt, cx)
    }

    /// Cancels an upload only if `attempt` is still the current path generation.
    pub fn cancel_attempt(&mut self, attempt: &UploadAttempt, cx: &mut Context<Self>) -> bool {
        self.cancel_generation(&attempt.path, attempt.generation, cx)
    }

    fn cancel_generation(&mut self, path: &Path, generation: u64, cx: &mut Context<Self>) -> bool {
        if !self.is_current_attempt(path, generation) {
            return false;
        }
        let Some((_, status)) = self.files.iter_mut().find(|(current, _)| current == path) else {
            return false;
        };
        if let Some(cancellation) = self.cancellations.get(path) {
            cancellation.cancel();
        }
        if !matches!(status, UploadStatus::Canceled) {
            *status = UploadStatus::Canceled;
            cx.emit(UploadEvent::Canceled {
                path: path.to_path_buf(),
            });
            cx.notify();
        }
        true
    }

    /// Resets a failed or cancelled path for a new attempt.
    pub fn retry_file(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        let Some((_, status)) = self.files.iter_mut().find(|(current, _)| current == path) else {
            return false;
        };
        if !matches!(status, UploadStatus::Failed(_) | UploadStatus::Canceled) {
            return false;
        }
        *status = UploadStatus::Ready;
        self.progress
            .insert(path.to_path_buf(), UploadProgress::default());
        self.cancellations.remove(path);
        self.attempts.remove(path);
        cx.emit(UploadEvent::Retried {
            path: path.to_path_buf(),
        });
        cx.notify();
        true
    }

    /// Removes a path from the queue and cancels any active attempt.
    pub fn remove_file(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.files.iter().position(|(current, _)| current == path) else {
            return false;
        };
        if let Some(cancellation) = self.cancellations.get(path) {
            cancellation.cancel();
        }
        self.files.remove(index);
        self.progress.remove(path);
        self.cancellations.remove(path);
        self.attempts.remove(path);
        cx.emit(UploadEvent::Removed {
            path: path.to_path_buf(),
        });
        cx.notify();
        true
    }

    /// Removes every queued path and requests cancellation for active attempts.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        let paths = self
            .files
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        for path in paths {
            self.remove_file(&path, cx);
        }
    }

    fn is_current_attempt(&self, path: &Path, attempt: u64) -> bool {
        self.attempts.get(path).copied() == Some(attempt)
            && self.files.iter().any(|(current, status)| {
                current == path && matches!(status, UploadStatus::Uploading)
            })
    }
}

async fn upload_paths_for_state(
    state: WeakEntity<UploadState>,
    paths: Vec<PathBuf>,
    backend: Arc<dyn UploadBackend>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    for path in paths {
        let Some(attempt) = state.update(cx, |state, cx| state.begin_attempt(&path, cx))? else {
            continue;
        };
        let cancellation = attempt.cancellation();
        let (progress_tx, progress_rx) = mpsc::unbounded();
        let request = UploadRequest::new(path.to_string_lossy().to_string())
            .cancellation(cancellation.clone())
            .total_bytes(std::fs::metadata(&path).ok().map(|metadata| metadata.len()))
            .on_progress(move |progress| {
                let _ = progress_tx.unbounded_send(progress);
            });
        let task = cx.update(|app| backend.upload_with_request(request, app))?;
        let mut upload_task = task.fuse();
        let mut progress_rx = progress_rx.fuse();
        let result = loop {
            select! {
                result = upload_task => break result,
                progress = progress_rx.next() => {
                    let Some(progress) = progress else {
                        break upload_task.await;
                    };
                    state.update(cx, |state, cx| {
                        state.set_attempt_progress(&attempt, progress, cx)
                    })?;
                }
            }
        };
        loop {
            let Some(Some(progress)) = progress_rx.next().now_or_never() else {
                break;
            };
            state.update(cx, |state, cx| {
                state.set_attempt_progress(&attempt, progress, cx)
            })?;
        }

        if attempt.is_cancelled() {
            state.update(cx, |state, cx| state.cancel_attempt(&attempt, cx))?;
        } else {
            match result {
                Ok(url) => state.update(cx, |state, cx| {
                    state.finish_attempt_success(&attempt, url, cx)
                })?,
                Err(error) => state.update(cx, |state, cx| {
                    state.finish_attempt_failure(&attempt, error.to_string(), cx)
                })?,
            }
        }
    }
    Ok(())
}

/// File-selection control. Transport is supplied through [`UploadBackend`].
#[derive(Clone, IntoElement)]
pub struct Upload {
    state: Entity<UploadState>,
    backend: Option<Arc<dyn UploadBackend>>,
    action: Option<SharedString>,
    multiple: bool,
    label: Option<SharedString>,
}
impl Upload {
    /// Creates an upload control.
    pub fn new(state: Entity<UploadState>) -> Self {
        Self {
            state,
            backend: None,
            action: None,
            multiple: false,
            label: None,
        }
    }
    /// Injects the upload implementation.
    pub fn backend(mut self, backend: Arc<dyn UploadBackend>) -> Self {
        self.backend = Some(backend);
        self
    }
    /// Configures TDesign's `action` URL using the built-in GPUI HTTP backend.
    pub fn action(mut self, action: impl Into<SharedString>) -> Self {
        self.action = Some(action.into());
        self
    }
    /// Enables multiple selection.
    pub fn multiple(mut self, value: bool) -> Self {
        self.multiple = value;
        self
    }
    /// Sets the visible selector label.
    pub fn label(mut self, value: impl Into<SharedString>) -> Self {
        self.label = Some(value.into());
        self
    }
    /// Returns the state entity used by this control.
    pub fn state(&self) -> Entity<UploadState> {
        self.state.clone()
    }
    fn resolved_backend(&self) -> Option<Arc<dyn UploadBackend>> {
        self.backend.clone().or_else(|| {
            self.action
                .clone()
                .map(|action| Arc::new(HttpUploadBackend::new(action)) as Arc<dyn UploadBackend>)
        })
    }
    /// Opens GPUI's native file prompt and returns its result channel.
    pub fn prompt_for_files(
        cx: &App,
        multiple: bool,
        prompt: impl Into<SharedString>,
    ) -> oneshot::Receiver<anyhow::Result<Option<Vec<PathBuf>>>> {
        cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple,
            prompt: Some(prompt.into()),
        })
    }

    /// Prompts for paths, adds them to state, and starts uploads when a backend exists.
    pub fn choose_files(&self, cx: &mut App) -> Task<anyhow::Result<Option<Vec<PathBuf>>>> {
        let receiver = Self::prompt_for_files(
            cx,
            self.multiple,
            self.label
                .clone()
                .unwrap_or_else(|| crate::locale::text(cx, "choose-file")),
        );
        let state = self.state.downgrade();
        let multiple = self.multiple;
        let backend = self.resolved_backend();
        cx.spawn(async move |async_cx| {
            let selected = receiver
                .await
                .map_err(|_| anyhow::anyhow!("file picker was closed unexpectedly"))??;
            let Some(paths) = selected else {
                return Ok(None);
            };
            let accepted =
                state.update(async_cx, |state, cx| state.add_files(paths, multiple, cx))?;
            if let Some(backend) = backend {
                upload_paths_for_state(state, accepted.clone(), backend, async_cx).await?;
            }
            Ok(Some(accepted))
        })
    }

    /// Adds paths and starts uploads immediately when a backend exists.
    pub fn upload_paths(
        &self,
        paths: impl IntoIterator<Item = PathBuf>,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let accepted = self
            .state
            .update(cx, |state, cx| state.add_files(paths, self.multiple, cx));
        let Some(backend) = self.resolved_backend() else {
            return Task::ready(Ok(()));
        };
        let state = self.state.downgrade();
        cx.spawn(async move |async_cx| {
            upload_paths_for_state(state, accepted, backend, async_cx).await
        })
    }

    /// Retries a failed or cancelled path.
    pub fn retry(&self, path: impl Into<PathBuf>, cx: &mut App) -> Task<anyhow::Result<()>> {
        let path = path.into();
        let accepted = self
            .state
            .update(cx, |state, cx| state.retry_file(&path, cx));
        let Some(backend) = self.resolved_backend() else {
            return Task::ready(Ok(()));
        };
        if !accepted {
            return Task::ready(Ok(()));
        }
        let state = self.state.downgrade();
        cx.spawn(async move |async_cx| {
            upload_paths_for_state(state, vec![path], backend, async_cx).await
        })
    }

    /// Requests cancellation of a path.
    pub fn cancel(&self, path: impl AsRef<std::path::Path>, cx: &mut App) -> bool {
        self.state
            .update(cx, |state, cx| state.cancel_file(path.as_ref(), cx))
    }
}
impl RenderOnce for Upload {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let disabled = state.disabled;
        let multiple = self.multiple;
        let state_entity = self.state.clone();
        let backend = self.resolved_backend();
        let button_state = state_entity.clone();
        let button_backend = backend.clone();
        let button_label = self
            .label
            .clone()
            .unwrap_or_else(|| crate::locale::text(cx, "choose-file"));
        let upload_ready = crate::locale::text(cx, "upload-ready");
        let upload_uploading = crate::locale::text(cx, "upload-uploading");
        let upload_success = crate::locale::text(cx, "upload-success");
        let upload_failed = crate::locale::text(cx, "upload-failed");
        let upload_canceled = crate::locale::text(cx, "upload-canceled");
        let cancel_text = crate::locale::text(cx, "cancel");
        let retry_text = crate::locale::text(cx, "retry");
        let mut root = div()
            .id(entity_id("tdesign-upload", &self.state))
            .flex()
            .flex_col()
            .gap_2()
            .items_start()
            .child(
                div()
                    .id(entity_id("tdesign-upload-trigger", &button_state))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .h_8()
                    .rounded_sm()
                    .border_1()
                    .border_color(gpui::rgb(0xdcdcdc))
                    .when(disabled, |this| this.opacity(0.5))
                    .child("⇧")
                    .child(button_label.clone())
                    .on_click(move |_: &ClickEvent, _, cx| {
                        if !button_state.read(cx).disabled {
                            Self::choose_files(
                                &Self {
                                    state: button_state.clone(),
                                    backend: button_backend.clone(),
                                    action: None,
                                    multiple,
                                    label: Some(button_label.clone()),
                                },
                                cx,
                            )
                            .detach();
                        }
                    }),
            );

        let rows = state
            .files
            .iter()
            .map(|(path, status)| {
                let path_for_action = path.clone();
                let action_state = state_entity.clone();
                let action_backend = backend.clone();
                let path_id = path.to_string_lossy();
                let progress = state.progress.get(path).copied().unwrap_or_default();
                let status_label: SharedString = match status {
                    UploadStatus::Ready => upload_ready.clone(),
                    UploadStatus::Uploading => progress
                        .fraction()
                        .map(|fraction| format!("{} {:.0}%", upload_uploading, fraction * 100.))
                        .unwrap_or_else(|| upload_uploading.to_string())
                        .into(),
                    UploadStatus::Success(_) => upload_success.clone(),
                    UploadStatus::Failed(_) => upload_failed.clone(),
                    UploadStatus::Canceled => upload_canceled.clone(),
                };
                let file_name: SharedString = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| path.to_string_lossy().into_owned())
                    .into();
                let action = match status {
                    UploadStatus::Uploading => {
                        let path = path_for_action.clone();
                        div()
                            .child(cancel_text.clone())
                            .text_color(gpui::rgb(0x0052d9))
                            .id(SharedString::from(format!(
                                "tdesign-upload-cancel-{:?}-{path_id}",
                                action_state.entity_id()
                            )))
                            .on_click(move |_, _, cx| {
                                let _ = action_state
                                    .update(cx, |state, cx| state.cancel_file(&path, cx));
                            })
                            .into_any_element()
                    }
                    UploadStatus::Failed(_) | UploadStatus::Canceled => {
                        let path = path_for_action.clone();
                        div()
                            .child(retry_text.clone())
                            .text_color(gpui::rgb(0x0052d9))
                            .id(SharedString::from(format!(
                                "tdesign-upload-retry-{:?}-{path_id}",
                                action_state.entity_id()
                            )))
                            .on_click(move |_, _, cx| {
                                let accepted = action_state
                                    .update(cx, |state, cx| state.retry_file(&path, cx));
                                if accepted {
                                    if let Some(backend) = action_backend.clone() {
                                        let state = action_state.downgrade();
                                        let upload_path = path.clone();
                                        cx.spawn(async move |async_cx| {
                                            upload_paths_for_state(
                                                state,
                                                vec![upload_path],
                                                backend,
                                                async_cx,
                                            )
                                            .await
                                        })
                                        .detach();
                                    }
                                }
                            })
                            .into_any_element()
                    }
                    _ => {
                        let path = path_for_action;
                        div()
                            .child("×")
                            .text_color(gpui::rgb(0x666666))
                            .id(SharedString::from(format!(
                                "tdesign-upload-remove-{:?}-{path_id}",
                                action_state.entity_id()
                            )))
                            .on_click(move |_, _, cx| {
                                let _ = action_state
                                    .update(cx, |state, cx| state.remove_file(&path, cx));
                            })
                            .into_any_element()
                    }
                };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w(px(280.))
                    .child(file_name)
                    .child(status_label)
                    .child(action)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        root = root.children(rows);
        root
    }
}
/// Upload module.
pub mod upload {
    pub use super::{Upload, UploadAttempt, UploadEvent, UploadState, UploadStatus};
    pub use crate::{
        HttpUploadBackend, UploadBackend, UploadCancellation, UploadProgress, UploadRequest,
    };
}
