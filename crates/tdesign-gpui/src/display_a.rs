//! Data-display components, part one.

use crate::{ComponentSize, Icon, ValueChange};
use chrono::{Datelike, Duration, NaiveDate};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Hsla, ImageSource, IntoElement, ParentElement,
    RenderOnce, SharedString, Window, div, img, prelude::*, px,
};
use std::sync::Arc;

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

/// Avatar shape.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AvatarShape {
    /// Circular avatar.
    #[default]
    Circle,
    /// Rounded square avatar.
    Square,
}

/// User or object avatar with image and fallback initials.
#[derive(IntoElement)]
pub struct Avatar {
    label: SharedString,
    source: Option<SharedString>,
    shape: AvatarShape,
    size: ComponentSize,
    fallback: Option<AnyElement>,
}

impl Avatar {
    /// Creates an avatar from a label.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            source: None,
            shape: AvatarShape::Circle,
            size: ComponentSize::Medium,
            fallback: None,
        }
    }

    /// Sets an image URL or asset path.
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets shape.
    pub fn shape(mut self, shape: AvatarShape) -> Self {
        self.shape = shape;
        self
    }

    /// Sets component size.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Sets custom fallback content.
    pub fn fallback(mut self, fallback: impl IntoElement) -> Self {
        self.fallback = Some(fallback.into_any_element());
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let dimension = self.size.height();
        let fallback = self.fallback.unwrap_or_else(|| {
            let initials = self
                .label
                .split_whitespace()
                .filter_map(|word| word.chars().next())
                .take(2)
                .collect::<String>();
            div()
                .child(if initials.is_empty() {
                    self.label.clone()
                } else {
                    initials.into()
                })
                .into_any_element()
        });
        let mut avatar = div()
            .id(SharedString::from(format!("tdesign-avatar-{}", self.label)))
            .flex()
            .items_center()
            .justify_center()
            .w(dimension)
            .h(dimension)
            .overflow_hidden()
            .when(self.shape == AvatarShape::Circle, |this| {
                this.rounded_full()
            })
            .when(self.shape == AvatarShape::Square, |this| this.rounded_sm())
            .bg(gpui::rgb(0xe8f1ff))
            .text_color(gpui::rgb(0x0052d9));
        if let Some(source) = self.source {
            avatar = avatar.child(img(source).w(dimension).h(dimension));
        } else {
            avatar = avatar.child(fallback);
        }
        avatar
    }
}

/// Avatar component module.
pub mod avatar {
    pub use super::{Avatar, AvatarShape};
}

/// Count badge attached to arbitrary content.
#[derive(IntoElement)]
pub struct Badge {
    child: AnyElement,
    count: Option<SharedString>,
    dot: bool,
    max: Option<usize>,
    color: Hsla,
}

impl Badge {
    /// Creates a badge around content.
    pub fn new(child: impl IntoElement) -> Self {
        Self {
            child: child.into_any_element(),
            count: None,
            dot: false,
            max: Some(99),
            color: gpui::rgb(0xd54941).into(),
        }
    }

    /// Sets count text.
    pub fn count(mut self, count: impl Into<SharedString>) -> Self {
        self.count = Some(count.into());
        self
    }

    /// Uses a dot without a count.
    pub fn dot(mut self, dot: bool) -> Self {
        self.dot = dot;
        self
    }

    /// Sets maximum numeric count before displaying a plus sign.
    pub fn max(mut self, max: usize) -> Self {
        self.max = Some(max);
        self
    }

    /// Sets badge color.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = color.into();
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let count = self.count.map(|count| {
            let display = count
                .parse::<usize>()
                .ok()
                .zip(self.max)
                .map(|(value, max)| {
                    if value > max {
                        format!("{max}+")
                    } else {
                        value.to_string()
                    }
                })
                .unwrap_or_else(|| count.to_string());
            div()
                .absolute()
                .top_0()
                .right_0()
                .min_w(px(16.))
                .h(px(16.))
                .px_1()
                .rounded_full()
                .bg(self.color)
                .text_color(gpui::white())
                .text_xs()
                .flex()
                .items_center()
                .justify_center()
                .child(display)
        });
        let dot = self.dot.then(|| {
            div()
                .absolute()
                .top_0()
                .right_0()
                .size_2()
                .rounded_full()
                .bg(self.color)
        });
        div()
            .id("tdesign-badge")
            .relative()
            .child(self.child)
            .children(count)
            .children(dot)
    }
}

/// Badge component module.
pub mod badge {
    pub use super::Badge;
}

/// Calendar selection state.
#[derive(Debug)]
pub struct CalendarState {
    /// Selected date.
    pub value: Option<NaiveDate>,
    /// First day of the visible month.
    pub month: NaiveDate,
    /// Earliest selectable date.
    pub min: Option<NaiveDate>,
    /// Latest selectable date.
    pub max: Option<NaiveDate>,
    /// Date currently highlighted for keyboard activation.
    pub highlighted: Option<NaiveDate>,
    /// Disabled state.
    pub disabled: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl CalendarState {
    /// Creates a calendar centered on a date.
    pub fn new(cx: &mut App, value: Option<NaiveDate>) -> Entity<Self> {
        let today = chrono::Local::now().date_naive();
        let selected = value.or(Some(today));
        let month = selected
            .map(|date| date.with_day(1).expect("valid first day"))
            .unwrap_or_else(|| today.with_day(1).expect("valid first day"));
        cx.new(|cx| Self {
            value,
            month,
            min: None,
            max: None,
            highlighted: selected,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Returns whether a date is unavailable for selection.
    pub fn is_date_disabled(&self, date: NaiveDate) -> bool {
        self.disabled
            || self.min.is_some_and(|min| date < min)
            || self.max.is_some_and(|max| date > max)
    }

    /// Selects an enabled date and returns the typed value change.
    pub fn select(
        &mut self,
        date: NaiveDate,
        cx: &mut Context<Self>,
    ) -> ValueChange<Option<NaiveDate>> {
        let previous = self.value;
        if !self.is_date_disabled(date) {
            self.value = Some(date);
            self.highlighted = Some(date);
            self.month = date.with_day(1).expect("valid first day");
            if previous != self.value {
                cx.notify();
            }
        }
        ValueChange {
            previous,
            current: self.value,
        }
    }

    /// Moves the keyboard highlight by a number of days, skipping disabled
    /// dates and keeping the visible month in sync.
    pub fn move_highlight(&mut self, amount: i32, cx: &mut Context<Self>) -> Option<NaiveDate> {
        if self.disabled || amount == 0 {
            return self.highlighted;
        }
        let direction = amount.signum();
        let mut remaining = amount.unsigned_abs();
        let mut date = self.highlighted.or(self.value).unwrap_or(self.month);
        while remaining > 0 {
            date += Duration::days(direction as i64);
            if !self.is_date_disabled(date) {
                remaining -= 1;
            }
            if remaining > 0
                && (self.min.is_some_and(|min| date <= min && direction < 0)
                    || self.max.is_some_and(|max| date >= max && direction > 0))
            {
                break;
            }
        }
        if self.is_date_disabled(date) {
            return self.highlighted;
        }
        self.highlighted = Some(date);
        self.month = date.with_day(1).expect("valid first day");
        cx.notify();
        self.highlighted
    }

    /// Selects the highlighted enabled date.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> ValueChange<Option<NaiveDate>> {
        self.highlighted
            .map(|date| self.select(date, cx))
            .unwrap_or(ValueChange {
                previous: self.value,
                current: self.value,
            })
    }

    /// Moves the visible month by a signed number of months.
    pub fn shift_month(&mut self, amount: i32, cx: &mut Context<Self>) {
        let previous_month = self.month;
        let mut year = self.month.year();
        let mut month = self.month.month0() as i32 + amount;
        while month < 0 {
            year -= 1;
            month += 12;
        }
        while month >= 12 {
            year += 1;
            month -= 12;
        }
        let next = NaiveDate::from_ymd_opt(year, month as u32 + 1, 1).expect("valid month");
        let min_month = self.min.and_then(|date| date.with_day(1));
        let max_month = self.max.and_then(|date| date.with_day(1));
        self.month = next
            .max(min_month.unwrap_or(next))
            .min(max_month.unwrap_or(next));
        let (next_year, next_month) = if self.month.month() == 12 {
            (self.month.year() + 1, 1)
        } else {
            (self.month.year(), self.month.month() + 1)
        };
        let last_day = (NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .expect("valid next month")
            - Duration::days(1))
        .day();
        let day = self.highlighted.map(|date| date.day()).unwrap_or(1);
        let mut highlighted = self
            .month
            .with_day(day.min(last_day))
            .expect("valid highlighted day");
        if let Some(min) = self.min {
            highlighted = highlighted.max(min);
        }
        if let Some(max) = self.max {
            highlighted = highlighted.min(max);
        }
        self.highlighted = (highlighted.year() == self.month.year()
            && highlighted.month() == self.month.month())
        .then_some(highlighted);
        if previous_month != self.month {
            cx.notify();
        }
    }
}

/// Month grid with keyboard-focusable date cells.
#[derive(IntoElement)]
pub struct Calendar {
    state: Entity<CalendarState>,
    on_change: Option<Arc<dyn Fn(ValueChange<Option<NaiveDate>>, &mut Window, &mut App)>>,
}

impl Calendar {
    /// Creates a calendar.
    pub fn new(state: Entity<CalendarState>) -> Self {
        Self {
            state,
            on_change: None,
        }
    }

    /// Registers date changes.
    pub fn on_change(
        mut self,
        handler: impl Fn(ValueChange<Option<NaiveDate>>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Calendar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let month = state.month;
        let selected = state.value;
        let highlighted = state.highlighted;
        let disabled = state.disabled;
        let focus = state.focus_handle.clone();
        let previous = self.state.clone();
        let next = self.state.clone();
        let select_entity = self.state.clone();
        let key_entity = self.state.clone();
        let key_handler = self.on_change.clone();
        let handler = self.on_change;
        let weekdays = ["日", "一", "二", "三", "四", "五", "六"]
            .into_iter()
            .map(|day| div().flex_1().text_center().text_sm().child(day))
            .collect::<Vec<_>>();
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
                    .id(SharedString::from(format!("calendar-{date}")))
                    .flex()
                    .items_center()
                    .justify_center()
                    .h_8()
                    .rounded_sm()
                    .when(!in_month, |this| this.text_color(gpui::rgb(0xbdbdbd)))
                    .when(is_selected, |this| {
                        this.bg(gpui::rgb(0x0052d9)).text_color(gpui::white())
                    })
                    .when(is_highlighted && !is_selected, |this| {
                        this.border_1().border_color(gpui::rgb(0x0052d9))
                    })
                    .when(date_disabled, |this| {
                        this.opacity(0.45).text_color(gpui::rgb(0x999999))
                    })
                    .when(!date_disabled, |this| {
                        this.hover(|this| this.bg(gpui::rgb(0xe8f1ff)))
                    })
                    .child(date.day().to_string())
                    .on_click(move |_, window, cx| {
                        if entity.read(cx).is_date_disabled(date) {
                            return;
                        }
                        let event = entity.update(cx, |state, cx| state.select(date, cx));
                        if event.previous != event.current
                            && let Some(handler) = &handler
                        {
                            handler(event, window, cx);
                        }
                    })
                    .into_any_element(),
            );
        }
        let month_title: SharedString = format!("{}年{}月", month.year(), month.month()).into();
        div()
            .id(entity_id("tdesign-calendar", &self.state))
            .track_focus(&focus)
            .w(px(320.))
            .p_3()
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .bg(gpui::white())
            .when(disabled, |this| this.opacity(0.5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .child(
                        div()
                            .id(entity_id("calendar-previous", &self.state))
                            .px_2()
                            .child("‹")
                            .on_click(move |_, _, cx| {
                                let _ = previous.update(cx, |state, cx| state.shift_month(-1, cx));
                            }),
                    )
                    .child(month_title)
                    .child(
                        div()
                            .id(entity_id("calendar-next", &self.state))
                            .px_2()
                            .child("›")
                            .on_click(move |_, _, cx| {
                                let _ = next.update(cx, |state, cx| state.shift_month(1, cx));
                            }),
                    ),
            )
            .child(div().flex().children(weekdays))
            .child(div().grid().grid_cols(7).gap_1().children(cells))
            .on_key_down(move |event, window, cx| {
                if key_entity.read(cx).disabled {
                    return;
                }
                let key = event.keystroke.key.to_ascii_lowercase();
                let handled = match key.as_str() {
                    "left" | "arrowleft" => key_entity.update(cx, |state, cx| {
                        state.move_highlight(-1, cx);
                        true
                    }),
                    "right" | "arrowright" => key_entity.update(cx, |state, cx| {
                        state.move_highlight(1, cx);
                        true
                    }),
                    "up" | "arrowup" => key_entity.update(cx, |state, cx| {
                        state.move_highlight(-7, cx);
                        true
                    }),
                    "down" | "arrowdown" => key_entity.update(cx, |state, cx| {
                        state.move_highlight(7, cx);
                        true
                    }),
                    "pageup" => key_entity.update(cx, |state, cx| {
                        state.shift_month(-1, cx);
                        true
                    }),
                    "pagedown" => key_entity.update(cx, |state, cx| {
                        state.shift_month(1, cx);
                        true
                    }),
                    "home" => key_entity.update(cx, |state, cx| {
                        let date = state.month;
                        state.highlighted = (!state.is_date_disabled(date)).then_some(date);
                        cx.notify();
                        true
                    }),
                    "end" => key_entity.update(cx, |state, cx| {
                        let (year, month) = if state.month.month() == 12 {
                            (state.month.year() + 1, 1)
                        } else {
                            (state.month.year(), state.month.month() + 1)
                        };
                        let date = NaiveDate::from_ymd_opt(year, month, 1)
                            .expect("valid next month")
                            - Duration::days(1);
                        state.highlighted = (!state.is_date_disabled(date)).then_some(date);
                        cx.notify();
                        true
                    }),
                    "enter" | "space" => {
                        let event = key_entity.update(cx, |state, cx| state.select_highlighted(cx));
                        if event.previous != event.current
                            && let Some(handler) = &key_handler
                        {
                            handler(event, window, cx);
                        }
                        true
                    }
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            })
    }
}

/// Calendar component module.
pub mod calendar {
    pub use super::{Calendar, CalendarState};
}

/// Card container with title, extra slot and body.
#[derive(IntoElement)]
pub struct Card {
    title: Option<SharedString>,
    subtitle: Option<SharedString>,
    extra: Option<AnyElement>,
    body: Option<AnyElement>,
    bordered: bool,
    hoverable: bool,
}

impl Card {
    /// Creates an empty card.
    pub fn new() -> Self {
        Self {
            title: None,
            subtitle: None,
            extra: None,
            body: None,
            bordered: true,
            hoverable: false,
        }
    }

    /// Sets card title.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets secondary title text.
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Sets the extra header slot.
    pub fn extra(mut self, extra: impl IntoElement) -> Self {
        self.extra = Some(extra.into_any_element());
        self
    }

    /// Sets body content.
    pub fn body(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }

    /// Controls the border.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Enables hover elevation.
    pub fn hoverable(mut self, hoverable: bool) -> Self {
        self.hoverable = hoverable;
        self
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let header =
            self.title.map(|title| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .child(div().flex().flex_col().gap_1().child(title).children(
                        self.subtitle.map(|subtitle| {
                            div()
                                .text_sm()
                                .text_color(gpui::rgb(0x777777))
                                .child(subtitle)
                        }),
                    ))
                    .children(self.extra)
            });
        div()
            .id("tdesign-card")
            .flex()
            .flex_col()
            .rounded_sm()
            .bg(gpui::white())
            .when(self.bordered, |this| {
                this.border_1().border_color(gpui::rgb(0xe7e7e7))
            })
            .when(self.hoverable, |this| this.hover(|this| this.shadow_md()))
            .children(header)
            .children(self.body.map(|body| div().p_4().child(body)))
    }
}

/// Card component module.
pub mod card {
    pub use super::Card;
}

/// One collapsible panel.
pub struct CollapseItem {
    /// Stable panel key.
    pub key: String,
    /// Header content.
    pub title: SharedString,
    /// Body content.
    pub content: AnyElement,
    /// Whether the panel starts open.
    pub expanded: bool,
    /// Disabled state.
    pub disabled: bool,
}

impl CollapseItem {
    /// Creates a panel.
    pub fn new(
        key: impl Into<String>,
        title: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            content: content.into_any_element(),
            expanded: false,
            disabled: false,
        }
    }
}

/// Expansion state for Collapse.
#[derive(Debug)]
pub struct CollapseState {
    /// Expanded keys.
    pub expanded: std::collections::BTreeSet<String>,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl CollapseState {
    /// Creates state from panels.
    pub fn new(cx: &mut App, items: &[CollapseItem]) -> Entity<Self> {
        let expanded = items
            .iter()
            .filter(|item| item.expanded)
            .map(|item| item.key.clone())
            .collect();
        cx.new(|cx| Self {
            expanded,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Toggles a panel key.
    pub fn toggle(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.to_owned());
        }
        cx.notify();
    }
}

/// Accordion-style collapsible panels.
#[derive(IntoElement)]
pub struct Collapse {
    state: Entity<CollapseState>,
    items: Vec<CollapseItem>,
    accordion: bool,
}

impl Collapse {
    /// Creates Collapse from panel definitions.
    pub fn new(state: Entity<CollapseState>, items: Vec<CollapseItem>) -> Self {
        Self {
            state,
            items,
            accordion: false,
        }
    }

    /// Enables one-panel-at-a-time behavior.
    pub fn accordion(mut self, accordion: bool) -> Self {
        self.accordion = accordion;
        self
    }
}

impl RenderOnce for Collapse {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let expanded = self.state.read(cx).expanded.clone();
        let focus = self.state.read(cx).focus_handle.clone();
        let entity = self.state.clone();
        let accordion = self.accordion;
        let panels = self
            .items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let is_open = expanded.contains(&key);
                let header_entity = entity.clone();
                div()
                    .id(SharedString::from(format!("collapse-{key}")))
                    .border_b_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .child(
                        div()
                            .id(SharedString::from(format!("collapse-header-{key}")))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .when(item.disabled, |this| this.opacity(0.5))
                            .child(item.title)
                            .child(if is_open { "⌃" } else { "⌄" })
                            .on_click(move |_, _, cx| {
                                if item.disabled {
                                    return;
                                }
                                let _ = header_entity.update(cx, |state, cx| {
                                    if accordion {
                                        state.expanded.clear();
                                    }
                                    state.toggle(&key, cx);
                                });
                            }),
                    )
                    .when(is_open, |this| {
                        this.child(div().px_3().py_3().child(item.content))
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-collapse", &self.state))
            .track_focus(&focus)
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .children(panels)
    }
}

/// Collapse component module.
pub mod collapse {
    pub use super::{Collapse, CollapseItem, CollapseState};
}

/// Rich comment block.
#[derive(IntoElement)]
pub struct Comment {
    author: SharedString,
    content: SharedString,
    time: Option<SharedString>,
    avatar: Option<AnyElement>,
    replies: Option<AnyElement>,
}

impl Comment {
    /// Creates a comment.
    pub fn new(author: impl Into<SharedString>, content: impl Into<SharedString>) -> Self {
        Self {
            author: author.into(),
            content: content.into(),
            time: None,
            avatar: None,
            replies: None,
        }
    }

    /// Sets timestamp.
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = Some(time.into());
        self
    }

    /// Sets avatar slot.
    pub fn avatar(mut self, avatar: impl IntoElement) -> Self {
        self.avatar = Some(avatar.into_any_element());
        self
    }

    /// Sets reply content.
    pub fn replies(mut self, replies: impl IntoElement) -> Self {
        self.replies = Some(replies.into_any_element());
        self
    }
}

impl RenderOnce for Comment {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("tdesign-comment")
            .flex()
            .gap_3()
            .py_3()
            .child(self.avatar.unwrap_or_else(|| {
                div()
                    .size_8()
                    .rounded_full()
                    .bg(gpui::rgb(0xe8f1ff))
                    .into_any_element()
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.author)
                            .children(self.time.map(|time| {
                                div().text_sm().text_color(gpui::rgb(0x999999)).child(time)
                            })),
                    )
                    .child(self.content)
                    .children(self.replies.map(|replies| div().pt_2().child(replies))),
            )
    }
}

/// Comment component module.
pub mod comment {
    pub use super::Comment;
}

/// One description row.
pub struct DescriptionItem {
    /// Label.
    pub label: SharedString,
    /// Value slot.
    pub content: AnyElement,
}

impl DescriptionItem {
    /// Creates a description item.
    pub fn new(label: impl Into<SharedString>, content: impl IntoElement) -> Self {
        Self {
            label: label.into(),
            content: content.into_any_element(),
        }
    }
}

/// Key/value description table.
#[derive(IntoElement)]
pub struct Descriptions {
    items: Vec<DescriptionItem>,
    columns: usize,
    bordered: bool,
}

impl Descriptions {
    /// Creates a description table.
    pub fn new(items: Vec<DescriptionItem>) -> Self {
        Self {
            items,
            columns: 1,
            bordered: true,
        }
    }

    /// Sets the number of columns.
    pub fn columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// Controls cell borders.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }
}

impl RenderOnce for Descriptions {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let cells = self
            .items
            .into_iter()
            .flat_map(|item| {
                let label = div()
                    .px_3()
                    .py_2()
                    .bg(gpui::rgb(0xf3f3f3))
                    .when(self.bordered, |this| {
                        this.border_1().border_color(gpui::rgb(0xe7e7e7))
                    })
                    .child(item.label);
                let content = div()
                    .px_3()
                    .py_2()
                    .when(self.bordered, |this| {
                        this.border_1().border_color(gpui::rgb(0xe7e7e7))
                    })
                    .child(item.content);
                [label.into_any_element(), content.into_any_element()]
            })
            .collect::<Vec<_>>();
        div()
            .id("tdesign-descriptions")
            .grid()
            .grid_cols((self.columns.saturating_mul(2)).min(u16::MAX as usize) as u16)
            .children(cells)
    }
}

/// Descriptions component module.
pub mod descriptions {
    pub use super::{DescriptionItem, Descriptions};
}

/// Empty-state illustration and action.
#[derive(IntoElement)]
pub struct Empty {
    description: SharedString,
    icon: Option<Icon>,
    action: Option<AnyElement>,
}

impl Empty {
    /// Creates an empty state.
    pub fn new(description: impl Into<SharedString>) -> Self {
        Self {
            description: description.into(),
            icon: None,
            action: None,
        }
    }

    /// Sets illustration icon.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets action slot.
    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for Empty {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("tdesign-empty")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_8()
            .text_color(gpui::rgb(0x777777))
            .children(self.icon.map(|icon| icon.size(px(48.))))
            .child(self.description)
            .children(self.action)
    }
}

/// Empty component module.
pub mod empty {
    pub use super::Empty;
}

/// Image fit behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ImageFit {
    /// Preserve the complete image.
    #[default]
    Contain,
    /// Fill the box and crop.
    Cover,
    /// Stretch to the box.
    Fill,
}

/// GPUI image with TDesign fallback styling.
#[derive(IntoElement)]
pub struct Image {
    source: ImageSource,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    fit: ImageFit,
    alt: Option<SharedString>,
}

impl Image {
    /// Creates an image from a GPUI image source.
    pub fn new(source: impl Into<ImageSource>) -> Self {
        Self {
            source: source.into(),
            width: None,
            height: None,
            fit: ImageFit::Contain,
            alt: None,
        }
    }

    /// Sets width.
    pub fn width(mut self, width: impl Into<gpui::Pixels>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Sets height.
    pub fn height(mut self, height: impl Into<gpui::Pixels>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// Sets fit mode.
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    /// Sets an accessible alternative label.
    pub fn alt(mut self, alt: impl Into<SharedString>) -> Self {
        self.alt = Some(alt.into());
        self
    }
}

impl RenderOnce for Image {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut image = img(self.source);
        if let Some(width) = self.width {
            image = image.w(width);
        }
        if let Some(height) = self.height {
            image = image.h(height);
        }
        match self.fit {
            ImageFit::Contain => image.object_fit(gpui::ObjectFit::Contain),
            ImageFit::Cover => image.object_fit(gpui::ObjectFit::Cover),
            ImageFit::Fill => image.object_fit(gpui::ObjectFit::Fill),
        }
    }
}

/// Image component module.
pub mod image {
    pub use super::{Image, ImageFit};
}

/// Image viewer state.
#[derive(Debug)]
pub struct ImageViewerState {
    /// Current image index.
    pub index: usize,
    /// Whether the viewer is open.
    pub open: bool,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl ImageViewerState {
    /// Creates a closed viewer state.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            index: 0,
            open: false,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Opens at an index.
    pub fn open_at(&mut self, index: usize, count: usize, cx: &mut Context<Self>) {
        self.index = index.min(count.saturating_sub(1));
        self.open = count > 0;
        cx.notify();
    }

    /// Closes the viewer.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        cx.notify();
    }

    /// Moves by one image.
    pub fn shift(&mut self, amount: isize, count: usize, cx: &mut Context<Self>) {
        if count == 0 {
            return;
        }
        self.index = (self.index as isize + amount).rem_euclid(count as isize) as usize;
        cx.notify();
    }
}

/// Fullscreen-in-container image viewer.
#[derive(IntoElement)]
pub struct ImageViewer {
    state: Entity<ImageViewerState>,
    sources: Vec<ImageSource>,
}

impl ImageViewer {
    /// Creates a viewer.
    pub fn new(
        state: Entity<ImageViewerState>,
        sources: impl IntoIterator<Item = impl Into<ImageSource>>,
    ) -> Self {
        Self {
            state,
            sources: sources.into_iter().map(Into::into).collect(),
        }
    }
}

impl RenderOnce for ImageViewer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let open = state.open;
        let index = state.index;
        let focus = state.focus_handle.clone();
        let count = self.sources.len();
        let current = self.sources.get(index).cloned();
        let previous = self.state.clone();
        let next = self.state.clone();
        let close = self.state.clone();
        div()
            .id(entity_id("tdesign-image-viewer", &self.state))
            .track_focus(&focus)
            .relative()
            .when(open, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .id("tdesign-image-viewer-overlay")
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_4()
                        .bg(gpui::rgba(0x000000cc))
                        .child(
                            div()
                                .id("tdesign-image-viewer-prev")
                                .text_color(gpui::white())
                                .text_xl()
                                .child("‹")
                                .on_click(move |_, _, cx| {
                                    let _ =
                                        previous.update(cx, |state, cx| state.shift(-1, count, cx));
                                }),
                        )
                        .children(current.map(|source| img(source).max_w(px(640.)).max_h(px(480.))))
                        .child(
                            div()
                                .id("tdesign-image-viewer-next")
                                .text_color(gpui::white())
                                .text_xl()
                                .child("›")
                                .on_click(move |_, _, cx| {
                                    let _ = next.update(cx, |state, cx| state.shift(1, count, cx));
                                }),
                        )
                        .child(
                            div()
                                .id("tdesign-image-viewer-close")
                                .absolute()
                                .top_2()
                                .right_2()
                                .text_color(gpui::white())
                                .child("×")
                                .on_click(move |_, _, cx| {
                                    let _ = close.update(cx, |state, cx| state.close(cx));
                                }),
                        ),
                )
            })
    }
}

/// ImageViewer component module.
pub mod image_viewer {
    pub use super::{ImageViewer, ImageViewerState};
}
