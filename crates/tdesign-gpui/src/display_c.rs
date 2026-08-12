//! Data-display components, part three.

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Hsla, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use std::sync::Arc;

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

/// Timeline layout mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TimelineMode {
    /// Content is placed to the right of the line.
    #[default]
    Left,
    /// Content is placed to the left of the line.
    Right,
    /// Content alternates around the line.
    Alternate,
}

/// One timeline event.
#[derive(Clone, Debug)]
pub struct TimelineItem {
    /// Event title.
    pub title: SharedString,
    /// Optional detail.
    pub content: Option<SharedString>,
    /// Optional time label.
    pub time: Option<SharedString>,
    /// Marker color.
    pub color: Hsla,
}

impl TimelineItem {
    /// Creates an event.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            content: None,
            time: None,
            color: gpui::rgb(0x0052d9).into(),
        }
    }

    /// Sets detail text.
    pub fn content(mut self, content: impl Into<SharedString>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Sets time text.
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// Sets marker color.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = color.into();
        self
    }
}

/// Vertical event timeline.
#[derive(IntoElement)]
pub struct Timeline {
    items: Vec<TimelineItem>,
    mode: TimelineMode,
}

impl Timeline {
    /// Creates a timeline.
    pub fn new(items: impl IntoIterator<Item = TimelineItem>) -> Self {
        Self {
            items: items.into_iter().collect(),
            mode: TimelineMode::Left,
        }
    }

    /// Sets content placement.
    pub fn mode(mut self, mode: TimelineMode) -> Self {
        self.mode = mode;
        self
    }
}

impl RenderOnce for Timeline {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let item_count = self.items.len();
        let items =
            self.items
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    let content = div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(
                            self.mode == TimelineMode::Alternate && index % 2 == 1,
                            |this| this.items_end(),
                        )
                        .child(item.title)
                        .children(item.content.map(|content| {
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0x666666))
                                .child(content)
                        }))
                        .children(item.time.map(|time| {
                            div().text_xs().text_color(gpui::rgb(0x999999)).child(time)
                        }));
                    let marker = div()
                        .size_3()
                        .rounded_full()
                        .bg(item.color)
                        .border_2()
                        .border_color(gpui::white());
                    div()
                        .id(SharedString::from(format!("timeline-item-{index}")))
                        .relative()
                        .flex()
                        .gap_3()
                        .pb_5()
                        .when(self.mode == TimelineMode::Right, |this| {
                            this.flex_row_reverse()
                        })
                        .when(
                            self.mode == TimelineMode::Alternate && index % 2 == 1,
                            |this| this.flex_row_reverse(),
                        )
                        .child(div().flex().w_4().justify_center().child(marker))
                        .child(content)
                        .when(index + 1 < item_count, |this| {
                            this.child(
                                div()
                                    .absolute()
                                    .left(px(6.))
                                    .top(px(14.))
                                    .bottom_0()
                                    .w(px(1.))
                                    .bg(gpui::rgb(0xe7e7e7)),
                            )
                        })
                        .into_any_element()
                })
                .collect::<Vec<_>>();
        div()
            .id("tdesign-timeline")
            .flex()
            .flex_col()
            .children(items)
    }
}

/// Timeline component module.
pub mod timeline {
    pub use super::{Timeline, TimelineItem, TimelineMode};
}

/// Tooltip placement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TooltipPlacement {
    /// Above the trigger.
    Top,
    /// Below the trigger.
    #[default]
    Bottom,
    /// Before the trigger.
    Left,
    /// After the trigger.
    Right,
}

/// Tooltip visibility state.
#[derive(Debug)]
pub struct TooltipState {
    /// Whether the tooltip is visible.
    pub visible: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl TooltipState {
    /// Creates hidden tooltip state.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            visible: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Sets visibility.
    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.visible = visible;
        cx.notify();
    }
}

/// Hover/focus tooltip around a trigger slot.
#[derive(IntoElement)]
pub struct Tooltip {
    state: Entity<TooltipState>,
    trigger: AnyElement,
    content: SharedString,
    placement: TooltipPlacement,
}

impl Tooltip {
    /// Creates a tooltip.
    pub fn new(
        state: Entity<TooltipState>,
        trigger: impl IntoElement,
        content: impl Into<SharedString>,
    ) -> Self {
        Self {
            state,
            trigger: trigger.into_any_element(),
            content: content.into(),
            placement: TooltipPlacement::Bottom,
        }
    }

    /// Sets placement.
    pub fn placement(mut self, placement: TooltipPlacement) -> Self {
        self.placement = placement;
        self
    }
}

impl RenderOnce for Tooltip {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let visible = self.state.read(cx).visible;
        let focus = self.state.read(cx).focus_handle.clone();
        let entity = self.state.clone();
        let mut popup = div()
            .id("tdesign-tooltip-content")
            .absolute()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(gpui::rgb(0x1f1f1f))
            .text_color(gpui::white())
            .text_sm()
            .child(self.content);
        popup = match self.placement {
            TooltipPlacement::Top => popup.bottom_full().left_0().mb_1(),
            TooltipPlacement::Bottom => popup.top_full().left_0().mt_1(),
            TooltipPlacement::Left => popup.right_full().top_0().mr_1(),
            TooltipPlacement::Right => popup.left_full().top_0().ml_1(),
        };
        div()
            .id(entity_id("tdesign-tooltip", &self.state))
            .track_focus(&focus)
            .relative()
            .on_hover(move |hovered, _window, cx| {
                let _ = entity.update(cx, |state, cx| state.set_visible(*hovered, cx));
            })
            .child(self.trigger)
            .when(visible, |this| this.child(popup))
    }
}

/// Tooltip component module.
pub mod tooltip {
    pub use super::{Tooltip, TooltipPlacement, TooltipState};
}

/// Repeated watermark text over a content slot.
#[derive(IntoElement)]
pub struct Watermark {
    text: SharedString,
    content: AnyElement,
    rows: usize,
    columns: usize,
    opacity: f32,
    gap: gpui::Pixels,
    rotate_degrees: f32,
}

impl Watermark {
    /// Creates a watermark around content.
    pub fn new(text: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self {
            text: text.into(),
            content: content.into_any_element(),
            rows: 4,
            columns: 5,
            opacity: 0.12,
            gap: px(96.),
            rotate_degrees: -18.,
        }
    }

    /// Sets the grid dimensions.
    pub fn grid(mut self, rows: usize, columns: usize) -> Self {
        self.rows = rows.max(1);
        self.columns = columns.max(1);
        self
    }

    /// Sets watermark opacity.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0., 1.);
        self
    }

    /// Sets horizontal and vertical gap.
    pub fn gap(mut self, gap: impl Into<gpui::Pixels>) -> Self {
        self.gap = gap.into();
        self
    }

    /// Sets the requested rotation in degrees.
    ///
    /// GPUI 0.2.2 does not expose transforms for text elements, so the value
    /// is retained for forward compatibility while the native grid remains
    /// readable on all platforms.
    pub fn rotate(mut self, degrees: f32) -> Self {
        self.rotate_degrees = degrees;
        self
    }
}

impl RenderOnce for Watermark {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text = self.text.clone();
        let opacity = self.opacity;
        let gap = self.gap;
        let columns = self.columns;
        div()
            .id("tdesign-watermark")
            .relative()
            .overflow_hidden()
            .child(self.content)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .gap_8()
                    .justify_center()
                    .children((0..self.rows).map(move |row| {
                        let row_items = (0..columns)
                            .map(|column| {
                                div()
                                    .id(SharedString::from(format!("watermark-{row}-{column}")))
                                    .text_sm()
                                    .text_color(gpui::rgba(0x000000ff))
                                    .opacity(opacity)
                                    .w(gap)
                                    .child(text.clone())
                                    .into_any_element()
                            })
                            .collect::<Vec<_>>();
                        div()
                            .id(SharedString::from(format!("watermark-row-{row}")))
                            .flex()
                            .gap(gap)
                            .justify_center()
                            .children(row_items)
                    })),
            )
    }
}

/// Watermark component module.
pub mod watermark {
    pub use super::Watermark;
}

/// Rating state.
#[derive(Debug)]
pub struct RateState {
    /// Current rating.
    pub value: f32,
    /// Maximum stars.
    pub max: usize,
    /// Whether half values are accepted.
    pub allow_half: bool,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl RateState {
    /// Creates rating state.
    pub fn new(cx: &mut App, value: f32, max: usize) -> Entity<Self> {
        cx.new(|cx| Self {
            value: value.clamp(0., max.max(1) as f32),
            max: max.max(1),
            allow_half: false,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Assigns a rating, respecting half-value policy.
    pub fn set_value(&mut self, value: f32, cx: &mut Context<Self>) {
        let step = if self.allow_half { 0.5 } else { 1. };
        self.value = (value / step).round() * step;
        self.value = self.value.clamp(0., self.max as f32);
        cx.notify();
    }
}

/// Clickable star rating.
#[derive(IntoElement)]
pub struct Rate {
    state: Entity<RateState>,
    on_change: Option<Arc<dyn Fn(f32, &mut Window, &mut App)>>,
}

impl Rate {
    /// Creates a rating control.
    pub fn new(state: Entity<RateState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers rating changes.
    pub fn on_change(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Rate {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let value = state.value;
        let max = state.max;
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let handler = self.on_change;
        let stars = (1..=max)
            .map(|index| {
                let filled = value >= index as f32;
                let half = !filled && state.allow_half && value + 0.5 >= index as f32;
                let entity = entity.clone();
                let handler = handler.clone();
                div()
                    .id(SharedString::from(format!("rate-{index}")))
                    .text_xl()
                    .text_color(if filled || half {
                        gpui::rgb(0xf2c94c)
                    } else {
                        gpui::rgb(0xbdbdbd)
                    })
                    .when(disabled, |this| this.opacity(0.5))
                    .child(if filled {
                        "★"
                    } else if half {
                        "⯨"
                    } else {
                        "☆"
                    })
                    .on_click(move |_, window, cx| {
                        if entity.read(cx).disabled {
                            return;
                        }
                        let _ = entity.update(cx, |state, cx| state.set_value(index as f32, cx));
                        if let Some(handler) = &handler {
                            handler(entity.read(cx).value, window, cx);
                        }
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-rate", &self.state))
            .track_focus(&focus)
            .flex()
            .items_center()
            .gap_1()
            .children(stars)
    }
}

/// Rate component module.
pub mod rate {
    pub use super::{Rate, RateState};
}
