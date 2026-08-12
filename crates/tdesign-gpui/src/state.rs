//! Reusable GPUI entities and delegate contracts for stateful components.

use futures::AsyncReadExt as _;
use gpui::{
    App, Bounds, ClipboardItem, Context, Entity, EntityInputHandler, EventEmitter, FocusHandle,
    KeyDownEvent, Pixels, Point, ShapedLine, SharedString, Task, UTF16Selection, Window,
    WrappedLine, prelude::*, px,
};
use std::{
    fs::File,
    io::Read as _,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// A change emitted by controlled input components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueChange<T> {
    /// Previous value.
    pub previous: T,
    /// Current value.
    pub current: T,
}

/// Events emitted by [`InputState`] for text mutations from builders, the
/// keyboard, clipboard operations, and the native IME input handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEvent {
    /// The text value changed.
    Change(ValueChange<SharedString>),
}

/// State shared by Input, Textarea and InputNumber.
pub struct InputState {
    /// Current text value.
    pub value: SharedString,
    /// Placeholder shown while empty.
    pub placeholder: SharedString,
    /// Disabled state.
    pub disabled: bool,
    /// Focus handle used for keyboard traversal and focus restoration.
    pub focus_handle: FocusHandle,
    /// Whether line breaks are accepted.
    pub multiline: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_multiline_layout: Option<MultilineLayout>,
    last_bounds: Option<Bounds<Pixels>>,
}

/// Cached layout information for a multiline input.
///
/// GPUI's [`WrappedLine`] stores byte offsets relative to each paragraph. The
/// `line_starts` vector maps those paragraph-local offsets back to the input's
/// UTF-8 byte offsets so IME and mouse hit testing can use the same coordinate
/// system as [`InputState::value`].
#[derive(Clone, Debug)]
pub(crate) struct MultilineLayout {
    pub lines: Vec<WrappedLine>,
    pub line_starts: Vec<usize>,
    pub line_height: Pixels,
}
impl std::fmt::Debug for InputState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InputState")
            .field("value", &self.value)
            .field("placeholder", &self.placeholder)
            .field("disabled", &self.disabled)
            .field("multiline", &self.multiline)
            .field("selected_range", &self.selected_range)
            .field("marked_range", &self.marked_range)
            .finish_non_exhaustive()
    }
}
impl InputState {
    /// Creates an entity with an initial value.
    pub fn new(cx: &mut App, value: impl Into<SharedString>) -> Entity<Self> {
        let value = value.into();
        cx.new(|cx| {
            let cursor = value.len();
            Self {
                value,
                placeholder: SharedString::default(),
                disabled: false,
                focus_handle: cx.focus_handle(),
                multiline: false,
                selected_range: cursor..cursor,
                selection_reversed: false,
                marked_range: None,
                last_layout: None,
                last_multiline_layout: None,
                last_bounds: None,
            }
        })
    }
    /// Updates the value and notifies observers.
    pub fn set_value(
        &mut self,
        value: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        self.assign_value(value.into(), true, cx)
    }

    /// Synchronizes a composite control's internal editor without emitting a
    /// second public input event back into its owner state.
    pub(crate) fn sync_value(
        &mut self,
        value: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        self.assign_value(value.into(), false, cx)
    }

    fn assign_value(
        &mut self,
        value: SharedString,
        emit: bool,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let previous = self.value.clone();
        self.value = value;
        let cursor = self.value.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.last_multiline_layout = None;
        if previous != self.value {
            cx.notify();
        }
        if emit {
            self.emit_change(previous, cx)
        } else {
            ValueChange {
                previous,
                current: self.value.clone(),
            }
        }
    }
    /// Sets placeholder text.
    pub fn placeholder(mut self, value: impl Into<SharedString>) -> Self {
        self.placeholder = value.into();
        self
    }

    /// Enables or disables line breaks.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Current byte-range selection.
    pub fn selection(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    /// Sets a UTF-8 byte selection, clamped to valid character boundaries.
    pub fn set_selection(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let start = self.clamp_boundary(range.start);
        let end = self.clamp_boundary(range.end);
        self.selected_range = start.min(end)..start.max(end);
        self.selection_reversed = range.end < range.start;
        cx.notify();
    }

    /// Replaces the current selection without requiring a Window handle.
    pub fn insert_text(
        &mut self,
        text: impl AsRef<str>,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        self.replace_selection(text.as_ref(), cx)
    }

    /// Current IME marked byte range.
    pub fn marked_range(&self) -> Option<Range<usize>> {
        self.marked_range.clone()
    }

    pub(crate) fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub(crate) fn set_layout(&mut self, line: ShapedLine, bounds: Bounds<Pixels>) {
        self.last_layout = Some(line);
        self.last_multiline_layout = None;
        self.last_bounds = Some(bounds);
    }

    pub(crate) fn set_multiline_layout(
        &mut self,
        lines: Vec<WrappedLine>,
        line_starts: Vec<usize>,
        line_height: Pixels,
        bounds: Bounds<Pixels>,
    ) {
        self.last_multiline_layout = Some(MultilineLayout {
            lines,
            line_starts,
            line_height,
        });
        self.last_layout = None;
        self.last_bounds = Some(bounds);
    }

    pub(crate) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let key = event.keystroke.key.to_ascii_lowercase();
        let modifiers = &event.keystroke.modifiers;
        if modifiers.secondary() {
            match key.as_str() {
                "a" => {
                    self.selected_range = 0..self.value.len();
                    self.selection_reversed = false;
                    cx.notify();
                }
                "c" => self.copy(cx),
                "x" => {
                    self.copy(cx);
                    self.replace_selection("", cx);
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.replace_selection(&text, cx);
                    }
                }
                "home" => self.move_to(0, modifiers.shift, cx),
                "end" => self.move_to(self.value.len(), modifiers.shift, cx),
                _ => {}
            }
            return;
        }
        match key.as_str() {
            "left" => {
                let target = if self.selected_range.is_empty() {
                    self.previous_boundary(self.cursor_offset())
                } else {
                    self.selected_range.start
                };
                self.move_to(target, modifiers.shift, cx);
            }
            "right" => {
                let target = if self.selected_range.is_empty() {
                    self.next_boundary(self.cursor_offset())
                } else {
                    self.selected_range.end
                };
                self.move_to(target, modifiers.shift, cx);
            }
            "home" if self.multiline => self.move_to(
                self.current_line_bounds(self.cursor_offset()).0,
                modifiers.shift,
                cx,
            ),
            "end" if self.multiline => self.move_to(
                self.current_line_bounds(self.cursor_offset()).1,
                modifiers.shift,
                cx,
            ),
            "home" => self.move_to(0, modifiers.shift, cx),
            "end" => self.move_to(self.value.len(), modifiers.shift, cx),
            "up" if self.multiline => self.move_vertical(-1, modifiers.shift, cx),
            "down" if self.multiline => self.move_vertical(1, modifiers.shift, cx),
            "backspace" => {
                if self.selected_range.is_empty() {
                    let cursor = self.cursor_offset();
                    self.selected_range = self.previous_boundary(cursor)..cursor;
                }
                self.replace_selection("", cx);
            }
            "delete" => {
                if self.selected_range.is_empty() {
                    let cursor = self.cursor_offset();
                    self.selected_range = cursor..self.next_boundary(cursor);
                }
                self.replace_selection("", cx);
            }
            "enter" if self.multiline => {
                self.replace_selection("\n", cx);
            }
            _ => {
                let _ = window;
            }
        }
    }

    fn copy(&self, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.value[self.selected_range.clone()].to_owned(),
            ));
        }
    }

    fn move_to(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        let offset = offset.min(self.value.len());
        if extend {
            let anchor = if self.selection_reversed {
                self.selected_range.end
            } else {
                self.selected_range.start
            };
            self.selected_range = anchor.min(offset)..anchor.max(offset);
            self.selection_reversed = offset < anchor;
        } else {
            self.selected_range = offset..offset;
            self.selection_reversed = false;
        }
        cx.notify();
    }

    fn current_line_bounds(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.value.len());
        let start = self.value[..offset]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let end = self.value[offset..]
            .find('\n')
            .map(|index| offset + index)
            .unwrap_or(self.value.len());
        (start, end)
    }

    fn move_vertical(&mut self, direction: i32, extend: bool, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let (line_start, line_end) = self.current_line_bounds(cursor);
        let column = self.value[line_start..cursor].chars().count();

        let previous_start = line_start
            .checked_sub(1)
            .and_then(|index| self.value[..index].rfind('\n').map(|newline| newline + 1))
            .unwrap_or(0);
        let next_start = if line_end < self.value.len() {
            line_end + 1
        } else {
            self.value.len() + 1
        };

        let target_start = if direction < 0 {
            if line_start == 0 {
                return;
            }
            previous_start
        } else {
            if next_start > self.value.len() {
                return;
            }
            next_start
        };
        let target_end = self.value[target_start..]
            .find('\n')
            .map(|index| target_start + index)
            .unwrap_or(self.value.len());
        let target_offset = self.value[target_start..target_end]
            .char_indices()
            .nth(column)
            .map(|(index, _)| target_start + index)
            .unwrap_or(target_end);
        self.move_to(target_offset, extend, cx);
    }

    fn replace_selection(
        &mut self,
        text: &str,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let previous = self.value.clone();
        let text = if self.multiline {
            text.to_owned()
        } else {
            text.replace(['\r', '\n'], " ")
        };
        let range = self.selected_range.clone();
        let mut value = self.value.to_string();
        value.replace_range(range.clone(), &text);
        let cursor = range.start + text.len();
        self.value = value.into();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        if previous != self.value {
            cx.notify();
        }
        self.emit_change(previous, cx)
    }

    fn emit_change(
        &self,
        previous: SharedString,
        cx: &mut Context<Self>,
    ) -> ValueChange<SharedString> {
        let change = ValueChange {
            previous,
            current: self.value.clone(),
        };
        if change.previous != change.current {
            cx.emit(InputEvent::Change(change.clone()));
        }
        change
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.value
            .char_indices()
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn clamp_boundary(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.value.len());
        while offset > 0 && !self.value.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.value
            .char_indices()
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.value.len())
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for character in self.value.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += character.len_utf16();
            utf8_offset += character.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for character in self.value.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += character.len_utf8();
            utf16_offset += character.len_utf16();
        }
        utf16_offset
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
}

impl EventEmitter<InputEvent> for InputState {}

impl EntityInputHandler for InputState {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range);
        adjusted_range.replace(self.range_to_utf16(&range));
        Some(self.value[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if self.disabled && !ignore_disabled_input {
            return None;
        }
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        self.selected_range = range;
        self.replace_selection(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.value.clone();
        let range = range
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        let text = if self.multiline {
            new_text.to_owned()
        } else {
            new_text.replace(['\r', '\n'], " ")
        };
        let mut value = self.value.to_string();
        value.replace_range(range.clone(), &text);
        self.value = value.into();
        self.marked_range = (!text.is_empty()).then_some(range.start..range.start + text.len());
        self.selected_range = new_selected_range
            .as_ref()
            .map(|selected| self.range_from_utf16(selected))
            .map(|selected| range.start + selected.start..range.start + selected.end)
            .unwrap_or_else(|| {
                let cursor = range.start + text.len();
                cursor..cursor
            });
        self.selection_reversed = false;
        if previous != self.value {
            cx.notify();
        }
        self.emit_change(previous, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range);
        if self.multiline
            && let Some(layout) = self.last_multiline_layout.as_ref()
        {
            let start = multiline_position_for_offset(layout, range.start);
            let end = multiline_position_for_offset(layout, range.end);
            return Some(Bounds::from_corners(
                gpui::point(bounds.left() + start.x, bounds.top() + start.y),
                gpui::point(
                    bounds.left() + end.x.max(start.x + px(1.)),
                    bounds.top() + end.y + layout.line_height,
                ),
            ));
        }
        let line = self.last_layout.as_ref()?;
        Some(Bounds::from_corners(
            gpui::point(bounds.left() + line.x_for_index(range.start), bounds.top()),
            gpui::point(bounds.left() + line.x_for_index(range.end), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let local = bounds.localize(&point)?;
        if self.multiline
            && let Some(layout) = self.last_multiline_layout.as_ref()
        {
            let offset = multiline_offset_for_point(layout, local);
            return Some(self.offset_to_utf16(offset));
        }
        let line = self.last_layout.as_ref()?;
        let index = line.index_for_x(point.x - local.x)?;
        Some(self.offset_to_utf16(index))
    }
}

/// Returns a point relative to a multiline input for a UTF-8 byte offset.
pub(crate) fn multiline_position_for_offset(
    layout: &MultilineLayout,
    offset: usize,
) -> Point<Pixels> {
    let offset = offset.min(
        layout
            .line_starts
            .last()
            .zip(layout.lines.last())
            .map(|(start, line)| start + line.len())
            .unwrap_or_default(),
    );
    for (index, line) in layout.lines.iter().enumerate() {
        let start = layout.line_starts.get(index).copied().unwrap_or(0);
        let end = start + line.len();
        if offset <= end || index + 1 == layout.lines.len() {
            let local = offset.saturating_sub(start).min(line.len());
            let position = line
                .position_for_index(local, layout.line_height)
                .unwrap_or_else(|| gpui::point(gpui::px(0.), gpui::px(0.)));
            let paragraph_y = layout.lines[..index]
                .iter()
                .map(|line| line.size(layout.line_height).height)
                .fold(gpui::px(0.), |total, height| total + height);
            return gpui::point(position.x, paragraph_y + position.y);
        }
    }
    gpui::point(gpui::px(0.), gpui::px(0.))
}

/// Converts a point relative to a multiline input to a UTF-8 byte offset.
pub(crate) fn multiline_offset_for_point(layout: &MultilineLayout, point: Point<Pixels>) -> usize {
    let mut y = point.y.max(gpui::px(0.));
    for (index, line) in layout.lines.iter().enumerate() {
        let height = line.size(layout.line_height).height;
        if y <= height || index + 1 == layout.lines.len() {
            let local = line
                .closest_index_for_position(gpui::point(point.x, y), layout.line_height)
                .unwrap_or_else(|offset| offset)
                .min(line.len());
            return layout.line_starts.get(index).copied().unwrap_or(0) + local;
        }
        y -= height;
    }
    layout
        .line_starts
        .last()
        .zip(layout.lines.last())
        .map(|(start, line)| start + line.len())
        .unwrap_or_default()
}

/// A generic list delegate. Components can virtualize rows without depending
/// on a particular collection type.
pub trait ListDelegate: 'static {
    /// Number of rows.
    fn row_count(&self) -> usize;
    /// Renders a row. The returned element is only requested for visible rows.
    fn render_row(&self, index: usize, window: &mut gpui::Window, cx: &mut App)
    -> gpui::AnyElement;
}

/// Table data contract with visible-row rendering.
pub trait TableDelegate: ListDelegate {
    /// Number of columns.
    fn column_count(&self) -> usize;
    /// Header label for a column.
    fn column_name(&self, column: usize) -> SharedString;
}

/// A tree node with stable identity and recursive children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeNode {
    /// Stable node key.
    pub key: String,
    /// Display label.
    pub label: SharedString,
    /// Child nodes.
    pub children: Vec<Self>,
    /// Whether the node is expanded.
    pub expanded: bool,
    /// Whether the node is selectable.
    pub disabled: bool,
}
impl TreeNode {
    /// Creates a leaf or parent node.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            children: Vec::new(),
            expanded: false,
            disabled: false,
        }
    }
    /// Appends a child node.
    pub fn child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }
}

/// Upload integration point. Applications provide transport and auth policy.
pub trait UploadBackend: Send + Sync + 'static {
    /// Uploads a local path and resolves to the remote URL or identifier.
    fn upload(&self, path: SharedString, cx: &mut App) -> Task<anyhow::Result<SharedString>>;

    /// Uploads with progress and cooperative-cancellation context.
    ///
    /// Existing backends only need to implement [`Self::upload`]. Override this
    /// method when the transport can report byte progress or observe cancellation.
    fn upload_with_request(
        &self,
        request: UploadRequest,
        cx: &mut App,
    ) -> Task<anyhow::Result<SharedString>> {
        self.upload(request.path, cx)
    }
}

/// Cooperative cancellation handle shared by Upload state and its backend.
#[derive(Clone, Debug, Default)]
pub struct UploadCancellation {
    cancelled: Arc<AtomicBool>,
}
impl UploadCancellation {
    /// Creates a non-cancelled handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Backends should check [`Self::is_cancelled`]
    /// between chunks and before committing a response.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Returns whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Byte-level progress reported by an [`UploadBackend`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UploadProgress {
    /// Bytes consumed from the local file.
    pub uploaded_bytes: u64,
    /// Total local-file bytes, when known.
    pub total_bytes: Option<u64>,
}
impl UploadProgress {
    /// Creates a progress sample.
    pub const fn new(uploaded_bytes: u64, total_bytes: Option<u64>) -> Self {
        Self {
            uploaded_bytes,
            total_bytes,
        }
    }

    /// Returns a normalized `0.0..=1.0` fraction when a non-zero total is known.
    pub fn fraction(self) -> Option<f32> {
        self.total_bytes
            .filter(|total| *total > 0)
            .map(|total| (self.uploaded_bytes.min(total) as f64 / total as f64) as f32)
    }
}

type UploadProgressHandler = Arc<dyn Fn(UploadProgress) + Send + Sync>;

/// Context passed to progress-aware upload backends.
#[derive(Clone)]
pub struct UploadRequest {
    /// Local path encoded lossily for cross-platform transport implementations.
    pub path: SharedString,
    /// Cooperative cancellation handle for this attempt.
    pub cancellation: UploadCancellation,
    /// Expected local-file size, when known.
    pub total_bytes: Option<u64>,
    progress: Option<UploadProgressHandler>,
}
impl std::fmt::Debug for UploadRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UploadRequest")
            .field("path", &self.path)
            .field("cancellation", &self.cancellation)
            .field("total_bytes", &self.total_bytes)
            .finish_non_exhaustive()
    }
}
impl UploadRequest {
    /// Creates a request for a local path.
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            cancellation: UploadCancellation::new(),
            total_bytes: None,
            progress: None,
        }
    }

    /// Replaces the cancellation handle.
    pub fn cancellation(mut self, cancellation: UploadCancellation) -> Self {
        self.cancellation = cancellation;
        self
    }

    /// Records the expected local-file size.
    pub fn total_bytes(mut self, total_bytes: Option<u64>) -> Self {
        self.total_bytes = total_bytes;
        self
    }

    /// Registers a thread-safe progress sink used by Upload state.
    pub fn on_progress(mut self, handler: impl Fn(UploadProgress) + Send + Sync + 'static) -> Self {
        self.progress = Some(Arc::new(handler));
        self
    }

    /// Reports byte progress to the Upload state, if a sink was installed.
    pub fn report_progress(&self, uploaded_bytes: u64, total_bytes: Option<u64>) {
        if let Some(handler) = &self.progress {
            handler(UploadProgress::new(uploaded_bytes, total_bytes));
        }
    }
}

/// Default `action` transport backed by GPUI's application HTTP client.
#[derive(Clone, Debug)]
pub struct HttpUploadBackend {
    action: SharedString,
    field_name: SharedString,
    headers: Vec<(SharedString, SharedString)>,
}
impl HttpUploadBackend {
    /// Creates a multipart `POST` backend for an action URL.
    pub fn new(action: impl Into<SharedString>) -> Self {
        Self {
            action: action.into(),
            field_name: "file".into(),
            headers: Vec::new(),
        }
    }

    /// Sets the multipart form field name.
    pub fn field_name(mut self, field_name: impl Into<SharedString>) -> Self {
        self.field_name = field_name.into();
        self
    }

    /// Adds an HTTP header, such as an authorization token.
    pub fn header(mut self, name: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Returns the configured action URL.
    pub fn action(&self) -> &str {
        self.action.as_ref()
    }
}
impl UploadBackend for HttpUploadBackend {
    fn upload(&self, path: SharedString, cx: &mut App) -> Task<anyhow::Result<SharedString>> {
        let total_bytes = std::fs::metadata(path.as_ref())
            .ok()
            .map(|metadata| metadata.len());
        self.upload_with_request(UploadRequest::new(path).total_bytes(total_bytes), cx)
    }

    fn upload_with_request(
        &self,
        request: UploadRequest,
        cx: &mut App,
    ) -> Task<anyhow::Result<SharedString>> {
        let action = self.action.clone();
        let field_name = self.field_name.clone();
        let headers = self.headers.clone();
        let client = cx.http_client();
        cx.background_executor().spawn(async move {
            if request.cancellation.is_cancelled() {
                anyhow::bail!("upload cancelled");
            }

            let path = PathBuf::from(request.path.as_ref());
            let total_bytes = request
                .total_bytes
                .or_else(|| std::fs::metadata(&path).ok().map(|metadata| metadata.len()));
            let boundary = multipart_boundary();
            let body =
                multipart_body(&path, field_name.as_ref(), &boundary, total_bytes, &request)?;
            if request.cancellation.is_cancelled() {
                anyhow::bail!("upload cancelled");
            }

            let mut builder = gpui::http_client::Request::builder()
                .method(gpui::http_client::Method::POST)
                .uri(action.as_ref())
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("Content-Length", body.len().to_string());
            for (name, value) in headers {
                builder = builder.header(name.as_ref(), value.as_ref());
            }
            let request_message = builder
                .body(gpui::http_client::AsyncBody::from(body))
                .map_err(anyhow::Error::from)?;
            let response = client.send(request_message).await?;
            let status = response.status();
            let location = response
                .headers()
                .get(gpui::http_client::http::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let (_, mut response_body) = response.into_parts();
            let mut response_bytes = Vec::new();
            response_body.read_to_end(&mut response_bytes).await?;
            let response_text = String::from_utf8_lossy(&response_bytes).trim().to_owned();

            if request.cancellation.is_cancelled() {
                anyhow::bail!("upload cancelled");
            }
            if !status.is_success() {
                let detail = if response_text.is_empty() {
                    String::new()
                } else {
                    format!(": {response_text}")
                };
                anyhow::bail!("upload failed with HTTP {status}{detail}");
            }

            request.report_progress(total_bytes.unwrap_or_default(), total_bytes);
            Ok(location
                .filter(|value| !value.is_empty())
                .or_else(|| (!response_text.is_empty()).then_some(response_text))
                .unwrap_or_else(|| action.to_string())
                .into())
        })
    }
}

fn multipart_boundary() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("tdesign-gpui-{}-{timestamp}", std::process::id())
}

fn multipart_body(
    path: &Path,
    field_name: &str,
    boundary: &str,
    total_bytes: Option<u64>,
    request: &UploadRequest,
) -> anyhow::Result<Vec<u8>> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.bin")
        .replace(['\r', '\n', '"'], "_");
    let field_name = field_name.replace(['\r', '\n', '"'], "_");
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{file_name}\"\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", content_type(path)).as_bytes());

    let mut file = File::open(path)?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut uploaded_bytes = 0_u64;
    loop {
        if request.cancellation.is_cancelled() {
            anyhow::bail!("upload cancelled");
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..read]);
        uploaded_bytes = uploaded_bytes.saturating_add(read as u64);
        request.report_progress(uploaded_bytes, total_bytes);
    }
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Ok(body)
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("json") => "application/json",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("txt") => "text/plain; charset=utf-8",
        Some("webp") => "image/webp",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}
