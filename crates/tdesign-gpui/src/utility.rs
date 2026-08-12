//! Navigation helpers that depend on native positioning and focus state.

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use std::sync::Arc;

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

/// Edge used by an affixed element.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AffixEdge {
    /// Pin to the top edge.
    #[default]
    Top,
    /// Pin to the bottom edge.
    Bottom,
}

/// Positions content at a stable edge of its native containing block.
#[derive(IntoElement)]
pub struct Affix {
    child: AnyElement,
    edge: AffixEdge,
    offset: gpui::Pixels,
    enabled: bool,
}

impl Affix {
    /// Creates an affix around one child.
    pub fn new(child: impl IntoElement) -> Self {
        Self {
            child: child.into_any_element(),
            edge: AffixEdge::Top,
            offset: px(0.),
            enabled: true,
        }
    }

    /// Selects the pinned edge.
    pub fn edge(mut self, edge: AffixEdge) -> Self {
        self.edge = edge;
        self
    }

    /// Sets the distance from the selected edge.
    pub fn offset(mut self, offset: impl Into<gpui::Pixels>) -> Self {
        self.offset = offset.into();
        self
    }

    /// Enables or disables affixed positioning.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl RenderOnce for Affix {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().relative().child(
            div()
                .when(self.enabled, |this| this.absolute().left_0().right_0())
                .when(self.enabled && self.edge == AffixEdge::Top, |this| {
                    this.top(self.offset)
                })
                .when(self.enabled && self.edge == AffixEdge::Bottom, |this| {
                    this.bottom(self.offset)
                })
                .child(self.child),
        )
    }
}

/// Affix component module.
pub mod affix {
    pub use super::{Affix, AffixEdge};
}

/// One item in an Anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnchorItem {
    /// Stable application key.
    pub key: String,
    /// Display text.
    pub label: SharedString,
    /// Zero-based nesting level.
    pub level: usize,
    /// Whether activation is blocked.
    pub disabled: bool,
}

impl AnchorItem {
    /// Creates a top-level anchor item.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            level: 0,
            disabled: false,
        }
    }

    /// Sets the nesting level.
    pub fn level(mut self, level: usize) -> Self {
        self.level = level;
        self
    }

    /// Sets disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Controlled active-anchor state.
#[derive(Debug)]
pub struct AnchorState {
    /// Available anchor items.
    pub items: Vec<AnchorItem>,
    /// Active item key.
    pub active: Option<String>,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl AnchorState {
    /// Creates anchor state and selects the first enabled item.
    pub fn new(cx: &mut App, items: Vec<AnchorItem>) -> Entity<Self> {
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

    /// Activates an enabled key.
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
}

type KeyHandler = Arc<dyn Fn(&str, &mut Window, &mut App)>;

/// Vertical in-page navigation mapped to application-defined native targets.
#[derive(IntoElement)]
pub struct Anchor {
    state: Entity<AnchorState>,
    on_change: Option<KeyHandler>,
}

impl Anchor {
    /// Creates an anchor navigation view.
    pub fn new(state: Entity<AnchorState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers an activation callback.
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Anchor {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let active = state.active.clone();
        let focus = state.focus_handle.clone();
        let items = state.items.clone();
        let entity = self.state.clone();
        let handler = self.on_change.clone();
        let children = items
            .into_iter()
            .map(|item| {
                let selected = active.as_deref() == Some(item.key.as_str());
                let key = item.key.clone();
                let callback_key = key.clone();
                let entity = entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("tdesign-anchor-{key}")))
                    .pl(px(12. + item.level as f32 * 16.))
                    .pr_3()
                    .py_1()
                    .border_l_2()
                    .border_color(if selected {
                        gpui::rgb(0x0052d9)
                    } else {
                        gpui::rgb(0xe7e7e7)
                    })
                    .when(selected, |this| this.text_color(gpui::rgb(0x0052d9)))
                    .when(item.disabled, |this| this.opacity(0.5))
                    .child(item.label)
                    .on_click(move |_, window, cx| {
                        let changed =
                            entity.update(cx, |state, cx| state.activate(&callback_key, cx));
                        if changed {
                            if let Some(handler) = &handler {
                                handler(&callback_key, window, cx);
                            }
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-anchor", &self.state))
            .track_focus(&focus)
            .flex()
            .flex_col()
            .children(children)
    }
}

/// Anchor component module.
pub mod anchor {
    pub use super::{Anchor, AnchorItem, AnchorState};
}

/// Floating control that delegates native scroll restoration to the host.
#[derive(IntoElement)]
pub struct BackTop {
    label: SharedString,
    visible: bool,
    on_click: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
}

impl BackTop {
    /// Creates a back-to-top control.
    pub fn new() -> Self {
        Self {
            label: "↑".into(),
            visible: true,
            on_click: None,
        }
    }

    /// Sets visible content.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Controls threshold-derived visibility.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Registers the application's scroll-to-top action.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }
}

impl Default for BackTop {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for BackTop {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let handler = self.on_click;
        div()
            .id("tdesign-back-top")
            .when(!self.visible, |this| this.invisible())
            .flex()
            .items_center()
            .justify_center()
            .size_10()
            .rounded_full()
            .bg(gpui::white())
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .shadow_md()
            .hover(|this| this.text_color(gpui::rgb(0x0052d9)))
            .child(self.label)
            .on_click(move |_, window, cx| {
                if let Some(handler) = &handler {
                    handler(window, cx);
                }
            })
    }
}

/// BackTop component module.
pub mod back_top {
    pub use super::BackTop;
}

/// One selectable dropdown entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DropdownItem {
    /// Stable item key.
    pub key: String,
    /// Display label.
    pub label: SharedString,
    /// Whether selection is blocked.
    pub disabled: bool,
    /// Whether a separator follows the item.
    pub divider: bool,
}

impl DropdownItem {
    /// Creates a dropdown item.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
            divider: false,
        }
    }
}

/// Dropdown open/selection state.
#[derive(Debug)]
pub struct DropdownState {
    /// Available items.
    pub items: Vec<DropdownItem>,
    /// Whether the popup is open.
    pub open: bool,
    /// Last selected key.
    pub selected: Option<String>,
    /// Index of the item currently highlighted for keyboard activation.
    pub highlighted: Option<usize>,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl DropdownState {
    /// Creates closed dropdown state.
    pub fn new(cx: &mut App, items: Vec<DropdownItem>) -> Entity<Self> {
        cx.new(|cx| Self {
            items,
            open: false,
            selected: None,
            highlighted: None,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Toggles popup visibility.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        if self.open && self.highlighted.is_none() {
            self.highlighted = self
                .selected
                .as_ref()
                .and_then(|key| self.items.iter().position(|item| &item.key == key))
                .or_else(|| self.items.iter().position(|item| !item.disabled));
        }
        cx.notify();
    }

    /// Moves the keyboard highlight, skipping disabled items and wrapping around.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.items.is_empty() {
            return;
        }
        let was_open = self.open;
        if !self.open {
            self.open = true;
        }
        let enabled = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!item.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return;
        }
        if !was_open {
            if delta < 0 {
                self.highlighted = enabled.last().copied();
            } else {
                self.highlighted = Some(enabled[0]);
            }
            cx.notify();
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

    /// Selects the highlighted item and closes the popup.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let index = self.highlighted?;
        let key = self.items.get(index)?.key.clone();
        self.select(&key, cx).then_some(key)
    }

    /// Selects an enabled item and closes the popup.
    pub fn select(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self
            .items
            .iter()
            .any(|item| item.key == key && !item.disabled)
        {
            self.selected = Some(key.to_owned());
            self.open = false;
            self.highlighted = self
                .selected
                .as_ref()
                .and_then(|selected| self.items.iter().position(|item| &item.key == selected));
            cx.notify();
            true
        } else {
            false
        }
    }
}

/// Native trigger and popup menu pair.
#[derive(IntoElement)]
pub struct Dropdown {
    state: Entity<DropdownState>,
    trigger: AnyElement,
    on_select: Option<KeyHandler>,
}

impl Dropdown {
    /// Creates a dropdown around a trigger element.
    pub fn new(state: Entity<DropdownState>, trigger: impl IntoElement) -> Self {
        Self {
            state,
            trigger: trigger.into_any_element(),
            on_select: None,
        }
    }

    /// Registers item selection.
    pub fn on_select(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Dropdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let open = state.open;
        let highlighted = state.highlighted;
        let focus = state.focus_handle.clone();
        let items = state.items.clone();
        let toggle_entity = self.state.clone();
        let select_entity = self.state.clone();
        let key_entity = self.state.clone();
        let handler = self.on_select.clone();
        let menu = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let key = item.key.clone();
                let entity = select_entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("tdesign-dropdown-{key}")))
                    .min_w(px(120.))
                    .px_3()
                    .py_2()
                    .when(item.disabled, |this| this.opacity(0.5))
                    .when(highlighted == Some(index), |this| {
                        this.bg(gpui::rgb(0xe8f1ff)).text_color(gpui::rgb(0x0052d9))
                    })
                    .when(!item.disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xf3f3f3)))
                    })
                    .child(item.label)
                    .when(item.divider, |this| {
                        this.border_b_1().border_color(gpui::rgb(0xe7e7e7))
                    })
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
            .id(entity_id("tdesign-dropdown", &self.state))
            .track_focus(&focus)
            .relative()
            .child(
                div()
                    .id(entity_id("tdesign-dropdown-trigger", &self.state))
                    .on_click(move |_, window, cx| {
                        toggle_entity.read(cx).focus_handle.focus(window);
                        let _ = toggle_entity.update(cx, |state, cx| state.toggle(cx));
                    })
                    .child(self.trigger),
            )
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt_1()
                        .py_1()
                        .rounded_sm()
                        .border_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(menu),
                )
            })
            .on_key_down(move |event, window, cx| {
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
                        let selected =
                            key_entity.update(cx, |state, cx| state.select_highlighted(cx));
                        if let Some(key) = selected
                            && let Some(handler) = &handler
                        {
                            handler(&key, window, cx);
                        }
                    }
                    "escape" => {
                        let _ = key_entity.update(cx, |state, cx| {
                            if state.open {
                                state.open = false;
                                cx.notify();
                            }
                        });
                    }
                    _ => {}
                }
            })
    }
}

/// Dropdown component module.
pub mod dropdown {
    pub use super::{Dropdown, DropdownItem, DropdownState};
}

/// One action in a StickyTool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StickyToolItem {
    /// Stable action key.
    pub key: String,
    /// Short visible label or glyph.
    pub label: SharedString,
    /// Optional hover description.
    pub description: Option<SharedString>,
    /// Disabled state.
    pub disabled: bool,
}

impl StickyToolItem {
    /// Creates a tool action.
    pub fn new(key: impl Into<String>, label: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            description: None,
            disabled: false,
        }
    }
}

/// Screen-edge tool group with application-defined actions.
#[derive(IntoElement)]
pub struct StickyTool {
    items: Vec<StickyToolItem>,
    on_select: Option<KeyHandler>,
}

impl StickyTool {
    /// Creates a sticky tool group.
    pub fn new(items: impl IntoIterator<Item = StickyToolItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            on_select: None,
        }
    }

    /// Registers an action callback.
    pub fn on_select(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for StickyTool {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let handler = self.on_select;
        let items = self
            .items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("tdesign-sticky-tool-{key}")))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size_10()
                    .border_b_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .when(item.disabled, |this| this.opacity(0.5))
                    .when(!item.disabled, |this| {
                        this.hover(|this| this.text_color(gpui::rgb(0x0052d9)))
                    })
                    .child(item.label)
                    .on_click(move |_, window, cx| {
                        if !item.disabled {
                            if let Some(handler) = &handler {
                                handler(&key, window, cx);
                            }
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("tdesign-sticky-tool")
            .flex()
            .flex_col()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .bg(gpui::white())
            .shadow_md()
            .overflow_hidden()
            .children(items)
    }
}

/// StickyTool component module.
pub mod sticky_tool {
    pub use super::{StickyTool, StickyToolItem};
}
