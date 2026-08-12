//! Stateful navigation components.

use crate::ValueChange;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*,
};
use std::sync::Arc;

type KeyHandler = Arc<dyn Fn(&str, &mut Window, &mut App)>;
type PageHandler = Arc<dyn Fn(ValueChange<usize>, &mut Window, &mut App)>;

/// One breadcrumb segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BreadcrumbItem {
    /// Stable route key.
    pub key: String,
    /// Visible text.
    pub label: SharedString,
}
impl BreadcrumbItem {
    /// Creates a segment.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
        }
    }
}

/// Hierarchical path navigation.
#[derive(Clone, Debug, IntoElement)]
pub struct Breadcrumb {
    items: Vec<BreadcrumbItem>,
    separator: SharedString,
}
impl Breadcrumb {
    /// Creates a breadcrumb.
    pub fn new(items: impl IntoIterator<Item = BreadcrumbItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            separator: "/".into(),
        }
    }
    /// Sets separator text.
    pub fn separator(mut self, value: impl Into<SharedString>) -> Self {
        self.separator = value.into();
        self
    }
}
impl RenderOnce for Breadcrumb {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut elements = Vec::<AnyElement>::new();
        let len = self.items.len();
        for (index, item) in self.items.into_iter().enumerate() {
            elements.push(
                div()
                    .text_color(if index + 1 == len {
                        gpui::rgb(0x1f1f1f)
                    } else {
                        gpui::rgb(0x666666)
                    })
                    .child(item.label)
                    .into_any_element(),
            );
            if index + 1 < len {
                elements.push(
                    div()
                        .px_1()
                        .text_color(gpui::rgb(0xbfbfbf))
                        .child(self.separator.clone())
                        .into_any_element(),
                );
            }
        }
        div().flex().items_center().children(elements)
    }
}
/// Breadcrumb module.
pub mod breadcrumb {
    pub use super::{Breadcrumb, BreadcrumbItem};
}

/// Menu orientation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MenuOrientation {
    #[default]
    Vertical,
    Horizontal,
}
/// One menu option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuItem {
    pub key: String,
    pub label: SharedString,
    pub disabled: bool,
}
impl MenuItem {
    /// Creates a menu item.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }
}
/// Menu selection state.
#[derive(Debug)]
pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected: Option<String>,
    pub highlighted: Option<usize>,
    pub orientation: MenuOrientation,
    pub focus_handle: FocusHandle,
}
impl MenuState {
    /// Creates menu state.
    pub fn new(cx: &mut App, items: Vec<MenuItem>) -> Entity<Self> {
        cx.new(|cx| {
            let highlighted = items.iter().position(|item| !item.disabled);
            Self {
                items,
                selected: None,
                highlighted,
                orientation: MenuOrientation::Vertical,
                focus_handle: cx.focus_handle(),
            }
        })
    }
    /// Selects an enabled item.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self
            .items
            .iter()
            .any(|item| item.key == key && !item.disabled)
        {
            self.selected = Some(key.to_owned());
            self.highlighted = self.items.iter().position(|item| item.key == key);
            cx.notify();
            true
        } else {
            false
        }
    }

    /// Moves the keyboard highlight, skipping disabled items and wrapping.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let enabled = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!item.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
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

    /// Highlights the first enabled menu item.
    pub fn highlight_first(&mut self, cx: &mut Context<Self>) {
        self.highlighted = self.items.iter().position(|item| !item.disabled);
        cx.notify();
    }

    /// Highlights the last enabled menu item.
    pub fn highlight_last(&mut self, cx: &mut Context<Self>) {
        self.highlighted = self.items.iter().rposition(|item| !item.disabled);
        cx.notify();
    }

    /// Selects the currently highlighted item.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let index = self.highlighted?;
        let key = self.items.get(index)?.key.clone();
        self.select(&key, cx).then_some(key)
    }
}
/// Vertical or horizontal menu.
#[derive(Clone, IntoElement)]
pub struct Menu {
    state: Entity<MenuState>,
    on_select: Option<KeyHandler>,
}
impl Menu {
    /// Creates a menu.
    pub fn new(state: Entity<MenuState>) -> Self {
        Self {
            state,
            on_select: None,
        }
    }

    /// Registers menu selection changes.
    pub fn on_select(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Menu {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selected.clone();
        let highlighted = state.highlighted;
        let orientation = state.orientation;
        let focus = state.focus_handle.clone();
        let items = state.items.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_select.clone();
        let handler = self.on_select;
        let children = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let key = item.key.clone();
                let entity = entity.clone();
                let handler = handler.clone();
                let active = selected.as_ref() == Some(&item.key);
                div()
                    .id(SharedString::from(format!("menu-{}", item.key)))
                    .px_3()
                    .py_2()
                    .rounded_sm()
                    .when(active, |this| {
                        this.bg(gpui::rgb(0xe7f1ff)).text_color(gpui::rgb(0x0052d9))
                    })
                    .when(highlighted == Some(index) && !active, |this| {
                        this.bg(gpui::rgb(0xf3f7ff))
                    })
                    .when(item.disabled, |this| this.opacity(0.5))
                    .child(item.label)
                    .on_click(move |_, window, cx| {
                        if entity.update(cx, |state, cx| state.select(&key, cx))
                            && let Some(handler) = &handler
                        {
                            handler(&key, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("tdesign-menu")
            .track_focus(&focus)
            .flex()
            .gap_1()
            .when(orientation == MenuOrientation::Vertical, |this| {
                this.flex_col()
            })
            .children(children)
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.to_ascii_lowercase();
                let activate = key_entity.update(cx, |state, cx| match key.as_str() {
                    "down" | "arrowdown" if state.orientation == MenuOrientation::Vertical => {
                        state.move_highlight(1, cx);
                        None
                    }
                    "up" | "arrowup" if state.orientation == MenuOrientation::Vertical => {
                        state.move_highlight(-1, cx);
                        None
                    }
                    "right" | "arrowright" if state.orientation == MenuOrientation::Horizontal => {
                        state.move_highlight(1, cx);
                        None
                    }
                    "left" | "arrowleft" if state.orientation == MenuOrientation::Horizontal => {
                        state.move_highlight(-1, cx);
                        None
                    }
                    "home" => {
                        state.highlight_first(cx);
                        None
                    }
                    "end" => {
                        state.highlight_last(cx);
                        None
                    }
                    "enter" | "space" => state.select_highlighted(cx),
                    _ => None,
                });
                if let Some(key) = activate
                    && let Some(handler) = &key_handler
                {
                    handler(&key, window, cx);
                }
            })
    }
}
/// Menu module.
pub mod menu {
    pub use super::{Menu, MenuItem, MenuOrientation, MenuState};
}

/// Paging state.
#[derive(Debug)]
pub struct PaginationState {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl PaginationState {
    /// Creates paging state.
    pub fn new(cx: &mut App, total: usize, page_size: usize) -> Entity<Self> {
        cx.new(|cx| Self {
            page: 1,
            page_size: page_size.max(1),
            total,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }
    /// Returns number of pages.
    pub fn page_count(&self) -> usize {
        self.total.div_ceil(self.page_size).max(1)
    }
    /// Selects a page after clamping it to the valid range.
    pub fn set_page(&mut self, page: usize, cx: &mut Context<Self>) -> ValueChange<usize> {
        let previous = self.page;
        self.page = page.clamp(1, self.page_count());
        if previous != self.page {
            cx.notify();
        }
        ValueChange {
            previous,
            current: self.page,
        }
    }

    /// Advances by one page.
    pub fn next(&mut self, cx: &mut Context<Self>) -> ValueChange<usize> {
        self.set_page(self.page.saturating_add(1), cx)
    }

    /// Moves back by one page.
    pub fn previous(&mut self, cx: &mut Context<Self>) -> ValueChange<usize> {
        self.set_page(self.page.saturating_sub(1), cx)
    }
}
/// Page navigation control.
#[derive(Clone, IntoElement)]
pub struct Pagination {
    state: Entity<PaginationState>,
    sibling_count: usize,
    on_change: Option<PageHandler>,
}
impl Pagination {
    /// Creates pagination.
    pub fn new(state: Entity<PaginationState>) -> Self {
        Self {
            state,
            sibling_count: 2,
            on_change: None,
        }
    }
    /// Sets the page window around the current page.
    pub fn sibling_count(mut self, count: usize) -> Self {
        self.sibling_count = count;
        self
    }

    /// Registers page changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Pagination {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let current = state.page;
        let count = state.page_count();
        let start = current.saturating_sub(self.sibling_count).max(1);
        let end = (current + self.sibling_count).min(count);
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let previous_entity = self.state.clone();
        let next_entity = self.state.clone();
        let key_entity = self.state.clone();
        let previous_handler = self.on_change.clone();
        let next_handler = self.on_change.clone();
        let key_handler = self.on_change.clone();
        let page_handler = self.on_change;
        let mut items = Vec::<AnyElement>::new();
        for page in start..=end {
            let entity = self.state.clone();
            let handler = page_handler.clone();
            items.push(
                div()
                    .id(page)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_8()
                    .h_8()
                    .rounded_sm()
                    .when(page == current, |this| {
                        this.bg(gpui::rgb(0x0052d9)).text_color(gpui::white())
                    })
                    .child(page.to_string())
                    .on_click(move |_, window, cx| {
                        let change = entity.update(cx, |state, cx| {
                            (!state.disabled).then(|| state.set_page(page, cx))
                        });
                        if let Some(change) = change
                            && let Some(handler) = &handler
                        {
                            handler(change, window, cx);
                        }
                    })
                    .into_any_element(),
            );
        }
        div()
            .id("tdesign-pagination")
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_1()
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .id("tdesign-pagination-previous")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_8()
                    .h_8()
                    .rounded_sm()
                    .child("‹")
                    .on_click(move |_, window, cx| {
                        let change = previous_entity.update(cx, |state, cx| {
                            (!state.disabled).then(|| state.previous(cx))
                        });
                        if let Some(change) = change
                            && let Some(handler) = &previous_handler
                        {
                            handler(change, window, cx);
                        }
                    }),
            )
            .children(items)
            .child(
                div()
                    .id("tdesign-pagination-next")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_8()
                    .h_8()
                    .rounded_sm()
                    .child("›")
                    .on_click(move |_, window, cx| {
                        let change = next_entity
                            .update(cx, |state, cx| (!state.disabled).then(|| state.next(cx)));
                        if let Some(change) = change
                            && let Some(handler) = &next_handler
                        {
                            handler(change, window, cx);
                        }
                    }),
            )
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.to_ascii_lowercase();
                let change = key_entity.update(cx, |state, cx| {
                    if state.disabled {
                        return None;
                    }
                    match key.as_str() {
                        "left" | "arrowleft" | "pageup" => Some(state.previous(cx)),
                        "right" | "arrowright" | "pagedown" => Some(state.next(cx)),
                        "home" => Some(state.set_page(1, cx)),
                        "end" => Some(state.set_page(state.page_count(), cx)),
                        _ => None,
                    }
                });
                if let Some(change) = change
                    && let Some(handler) = &key_handler
                {
                    handler(change, window, cx);
                }
            })
    }
}
/// Pagination module.
pub mod pagination {
    pub use super::{Pagination, PaginationState};
}

/// One step in a progress sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepItem {
    pub title: SharedString,
    pub description: Option<SharedString>,
}
impl StepItem {
    /// Creates a step.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
        }
    }
}
/// Progress indicator across ordered steps.
#[derive(Clone, Debug, IntoElement)]
pub struct Steps {
    items: Vec<StepItem>,
    current: usize,
    vertical: bool,
}
impl Steps {
    /// Creates steps.
    pub fn new(items: impl IntoIterator<Item = StepItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            current: 0,
            vertical: false,
        }
    }
    /// Sets zero-based active step.
    pub fn current(mut self, index: usize) -> Self {
        self.current = index;
        self
    }
    /// Enables vertical layout.
    pub fn vertical(mut self, value: bool) -> Self {
        self.vertical = value;
        self
    }
}
impl RenderOnce for Steps {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let current = self.current;
        let children = self
            .items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(self.vertical, |this| this.pb_4())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_6()
                            .h_6()
                            .rounded_full()
                            .bg(if index <= current {
                                gpui::rgb(0x0052d9)
                            } else {
                                gpui::rgb(0xdcdcdc)
                            })
                            .text_color(gpui::white())
                            .child((index + 1).to_string()),
                    )
                    .child(div().flex().flex_col().child(item.title).children(
                        item.description.map(|value| {
                            div().text_sm().text_color(gpui::rgb(0x888888)).child(value)
                        }),
                    ))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .when(self.vertical, |this| this.flex_col())
            .when(!self.vertical, |this| this.items_start().gap_4())
            .children(children)
    }
}
/// Steps module.
pub mod steps {
    pub use super::{StepItem, Steps};
}

/// One tab option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabItem {
    pub key: String,
    pub label: SharedString,
    pub disabled: bool,
}
impl TabItem {
    /// Creates a tab.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }
}
/// Tabs active-key state.
#[derive(Debug)]
pub struct TabsState {
    pub items: Vec<TabItem>,
    pub active: Option<String>,
    pub focus_handle: FocusHandle,
}
impl TabsState {
    /// Creates tabs state, selecting the first enabled tab.
    pub fn new(cx: &mut App, items: Vec<TabItem>) -> Entity<Self> {
        cx.new(|cx| {
            let active = items
                .iter()
                .find(|item| !item.disabled)
                .map(|item| item.key.clone());
            Self {
                items,
                active,
                focus_handle: cx.focus_handle(),
            }
        })
    }
    /// Changes the active tab.
    pub fn activate(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self
            .items
            .iter()
            .any(|item| item.key == key && !item.disabled)
        {
            self.active = Some(key.to_owned());
            cx.notify();
            true
        } else {
            false
        }
    }

    /// Moves activation through enabled tabs, wrapping around.
    pub fn move_active(&mut self, delta: i32, cx: &mut Context<Self>) -> Option<String> {
        let enabled = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!item.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return None;
        }
        let current = self
            .active
            .as_ref()
            .and_then(|key| self.items.iter().position(|item| &item.key == key))
            .and_then(|index| enabled.iter().position(|candidate| *candidate == index))
            .unwrap_or(0);
        let next = (current as i32 + delta).rem_euclid(enabled.len() as i32) as usize;
        let key = self.items[enabled[next]].key.clone();
        self.activate(&key, cx).then_some(key)
    }

    /// Activates the first enabled tab.
    pub fn activate_first(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let key = self.items.iter().find(|item| !item.disabled)?.key.clone();
        self.activate(&key, cx).then_some(key)
    }

    /// Activates the last enabled tab.
    pub fn activate_last(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let key = self
            .items
            .iter()
            .rev()
            .find(|item| !item.disabled)?
            .key
            .clone();
        self.activate(&key, cx).then_some(key)
    }
}
/// Tab navigation bar.
#[derive(Clone, IntoElement)]
pub struct Tabs {
    state: Entity<TabsState>,
    on_change: Option<KeyHandler>,
}
impl Tabs {
    /// Creates tabs.
    pub fn new(state: Entity<TabsState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers active-tab changes.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let active = state.active.clone();
        let focus = state.focus_handle.clone();
        let items = state.items.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let handler = self.on_change;
        let children = items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let entity = entity.clone();
                let handler = handler.clone();
                let selected = active.as_ref() == Some(&item.key);
                let border: gpui::Hsla = if selected {
                    gpui::rgb(0x0052d9).into()
                } else {
                    gpui::transparent_black()
                };
                div()
                    .id(SharedString::from(format!("tab-{}", item.key)))
                    .px_3()
                    .py_2()
                    .border_b_2()
                    .border_color(border)
                    .when(selected, |this| this.text_color(gpui::rgb(0x0052d9)))
                    .when(item.disabled, |this| this.opacity(0.5))
                    .child(item.label)
                    .on_click(move |_, window, cx| {
                        if entity.update(cx, |state, cx| state.activate(&key, cx))
                            && let Some(handler) = &handler
                        {
                            handler(&key, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("tdesign-tabs")
            .track_focus(&focus)
            .flex()
            .border_b_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .children(children)
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.to_ascii_lowercase();
                let active = key_entity.update(cx, |state, cx| match key.as_str() {
                    "right" | "arrowright" | "down" | "arrowdown" => state.move_active(1, cx),
                    "left" | "arrowleft" | "up" | "arrowup" => state.move_active(-1, cx),
                    "home" => state.activate_first(cx),
                    "end" => state.activate_last(cx),
                    _ => None,
                });
                if let Some(active) = active
                    && let Some(handler) = &key_handler
                {
                    handler(&active, window, cx);
                }
            })
    }
}
/// Tabs module.
pub mod tabs {
    pub use super::{TabItem, Tabs, TabsState};
}
