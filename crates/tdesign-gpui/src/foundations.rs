//! Foundation and layout components.

use crate::{ComponentSize, Disableable, Icon, Sizable};
use gpui::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use std::sync::Arc;

/// Text link with optional icon and native click handling.
#[derive(IntoElement)]
pub struct Link {
    label: SharedString,
    href: Option<SharedString>,
    icon: Option<Icon>,
    disabled: bool,
    size: ComponentSize,
    color: Option<Hsla>,
    on_click: Option<Arc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
}
impl Link {
    /// Creates a link.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            href: None,
            icon: None,
            disabled: false,
            size: ComponentSize::Medium,
            color: None,
            on_click: None,
        }
    }
    /// Sets a native URL opened with GPUI when the link is activated.
    pub fn href(mut self, href: impl Into<SharedString>) -> Self {
        self.href = Some(href.into());
        self
    }
    /// Adds a leading icon.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    /// Overrides link color.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
    /// Registers the activation handler.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }
}
impl Disableable for Link {
    fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}
impl Sizable for Link {
    fn size(mut self, value: ComponentSize) -> Self {
        self.size = value;
        self
    }
}
impl RenderOnce for Link {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled;
        let href = self.href.clone();
        let click_handler = self.on_click.clone();
        let mut link = div()
            .id(SharedString::from(format!("tdesign-link-{}", self.label)))
            .focusable()
            .flex()
            .items_center()
            .gap_1()
            .h(self.size.height())
            .text_color(self.color.unwrap_or_else(|| gpui::rgb(0x0052d9).into()))
            .when(self.disabled, |this| this.opacity(0.5))
            .when(!self.disabled, |this| {
                this.hover(|this| this.text_color(gpui::rgb(0x266fe8)))
            });
        if let Some(icon) = self.icon {
            link = link.child(icon);
        }
        link = link.child(self.label);
        if click_handler.is_some() || href.is_some() {
            link = link.on_click(move |event, window, cx| {
                if !disabled {
                    if let Some(handler) = &click_handler {
                        handler(event, window, cx);
                    }
                    if let Some(href) = &href {
                        cx.open_url(href);
                    }
                }
            });
        }
        link
    }
}
/// Link module.
pub mod link {
    pub use super::Link;
}

/// Semantic typography styles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TypographyVariant {
    #[default]
    Body,
    BodySmall,
    Title,
    Headline,
    Code,
    Mark,
}

/// Styled text with TDesign's desktop type scale.
#[derive(Clone, Debug, IntoElement)]
pub struct Typography {
    text: SharedString,
    variant: TypographyVariant,
    color: Option<Hsla>,
    truncate: bool,
}
impl Typography {
    /// Creates body text.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            variant: TypographyVariant::Body,
            color: None,
            truncate: false,
        }
    }
    /// Sets semantic style.
    pub fn variant(mut self, variant: TypographyVariant) -> Self {
        self.variant = variant;
        self
    }
    /// Sets text color.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
    /// Enables single-line ellipsis.
    pub fn truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }
}
impl RenderOnce for Typography {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut text = div().when(self.truncate, |this| {
            this.whitespace_nowrap().overflow_hidden().text_ellipsis()
        });
        text = match self.variant {
            TypographyVariant::BodySmall => text.text_sm(),
            TypographyVariant::Title => text.text_xl().font_weight(FontWeight::SEMIBOLD),
            TypographyVariant::Headline => text.text_2xl().font_weight(FontWeight::BOLD),
            TypographyVariant::Code => text.font_family("monospace"),
            TypographyVariant::Mark => text.bg(gpui::rgb(0xfff0b5)).px_1(),
            TypographyVariant::Body => text.text_base(),
        };
        if let Some(color) = self.color {
            text = text.text_color(color);
        }
        text.child(self.text)
    }
}
/// Typography module.
pub mod typography {
    pub use super::{Typography, TypographyVariant};
}

/// Divider orientation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DividerOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Horizontal or vertical content separator.
#[derive(Clone, Debug, IntoElement)]
pub struct Divider {
    orientation: DividerOrientation,
    label: Option<SharedString>,
    dashed: bool,
}
impl Divider {
    /// Creates a horizontal divider.
    pub fn new() -> Self {
        Self {
            orientation: DividerOrientation::Horizontal,
            label: None,
            dashed: false,
        }
    }
    /// Sets orientation.
    pub fn orientation(mut self, value: DividerOrientation) -> Self {
        self.orientation = value;
        self
    }
    /// Adds a centered label.
    pub fn label(mut self, value: impl Into<SharedString>) -> Self {
        self.label = Some(value.into());
        self
    }
    /// Enables a dashed line.
    pub fn dashed(mut self, value: bool) -> Self {
        self.dashed = value;
        self
    }
}
impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}
impl RenderOnce for Divider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        match self.orientation {
            DividerOrientation::Vertical => div()
                .h_5()
                .border_l_1()
                .border_color(gpui::rgb(0xe7e7e7))
                .when(self.dashed, |this| this.border_dashed())
                .into_any_element(),
            DividerOrientation::Horizontal => {
                let line = || {
                    div()
                        .flex_1()
                        .border_t_1()
                        .border_color(gpui::rgb(0xe7e7e7))
                        .when(self.dashed, |this| this.border_dashed())
                };
                let mut row = div()
                    .flex()
                    .items_center()
                    .w_full()
                    .gap_3()
                    .py_2()
                    .child(line());
                if let Some(label) = self.label {
                    row = row.child(label).child(line());
                }
                row.into_any_element()
            }
        }
    }
}
/// Divider module.
pub mod divider {
    pub use super::{Divider, DividerOrientation};
}

/// Native CSS-grid-like layout backed by GPUI/Taffy.
#[derive(IntoElement)]
pub struct Grid {
    columns: u16,
    rows: Option<u16>,
    gap: gpui::Pixels,
    children: Vec<AnyElement>,
}
impl Grid {
    /// Creates a grid with the requested column count.
    pub fn new(columns: u16) -> Self {
        Self {
            columns: columns.max(1),
            rows: None,
            gap: px(16.),
            children: Vec::new(),
        }
    }
    /// Sets explicit rows.
    pub fn rows(mut self, rows: u16) -> Self {
        self.rows = Some(rows.max(1));
        self
    }
    /// Sets row and column gap.
    pub fn gap(mut self, gap: impl Into<gpui::Pixels>) -> Self {
        self.gap = gap.into();
        self
    }
    /// Adds a grid item.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
    /// Adds grid items.
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.children
            .extend(children.into_iter().map(IntoElement::into_any_element));
        self
    }
}
impl RenderOnce for Grid {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .grid()
            .grid_cols(self.columns)
            .gap(self.gap)
            .when_some(self.rows, |this, rows| this.grid_rows(rows))
            .children(self.children)
    }
}
/// Grid module.
pub mod grid {
    pub use super::Grid;
}

/// Standard application layout with named header/aside/content/footer slots.
#[derive(IntoElement, Default)]
pub struct Layout {
    header: Option<AnyElement>,
    aside: Option<AnyElement>,
    content: Option<AnyElement>,
    footer: Option<AnyElement>,
}
impl Layout {
    /// Creates an empty layout.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets header slot.
    pub fn header(mut self, value: impl IntoElement) -> Self {
        self.header = Some(value.into_any_element());
        self
    }
    /// Sets aside slot.
    pub fn aside(mut self, value: impl IntoElement) -> Self {
        self.aside = Some(value.into_any_element());
        self
    }
    /// Sets content slot.
    pub fn content(mut self, value: impl IntoElement) -> Self {
        self.content = Some(value.into_any_element());
        self
    }
    /// Sets footer slot.
    pub fn footer(mut self, value: impl IntoElement) -> Self {
        self.footer = Some(value.into_any_element());
        self
    }
}
impl RenderOnce for Layout {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .children(self.header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .children(self.aside)
                    .child(div().flex_1().children(self.content)),
            )
            .children(self.footer)
    }
}
/// Layout module.
pub mod layout {
    pub use super::Layout;
}

/// Stack orientation for [`Space`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SpaceOrientation {
    #[default]
    Horizontal,
    Vertical,
}

/// Flex stack with consistent spacing.
#[derive(IntoElement)]
pub struct Space {
    orientation: SpaceOrientation,
    gap: gpui::Pixels,
    wrap: bool,
    children: Vec<AnyElement>,
}
impl Space {
    /// Creates a horizontal stack.
    pub fn new() -> Self {
        Self {
            orientation: SpaceOrientation::Horizontal,
            gap: px(8.),
            wrap: false,
            children: Vec::new(),
        }
    }
    /// Sets orientation.
    pub fn orientation(mut self, value: SpaceOrientation) -> Self {
        self.orientation = value;
        self
    }
    /// Sets spacing.
    pub fn gap(mut self, value: impl Into<gpui::Pixels>) -> Self {
        self.gap = value.into();
        self
    }
    /// Enables wrapping.
    pub fn wrap(mut self, value: bool) -> Self {
        self.wrap = value;
        self
    }
    /// Adds a child.
    pub fn child(mut self, value: impl IntoElement) -> Self {
        self.children.push(value.into_any_element());
        self
    }
}
impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}
impl RenderOnce for Space {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .gap(self.gap)
            .when(self.orientation == SpaceOrientation::Vertical, |this| {
                this.flex_col()
            })
            .when(self.wrap, |this| this.flex_wrap())
            .children(self.children)
    }
}
/// Space module.
pub mod space {
    pub use super::{Space, SpaceOrientation};
}
