//! Data-display components, part two.

use crate::ComponentSize;
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Hsla, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px,
};
use qrcodegen::{QrCode, QrCodeEcc};
use std::sync::Arc;

fn entity_id<T: 'static>(prefix: &str, entity: &Entity<T>) -> SharedString {
    format!("{prefix}-{:?}", entity.entity_id()).into()
}

/// Loading indicator style.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LoadingVariant {
    /// Rotating glyph-like spinner.
    #[default]
    Spinner,
    /// Three-dot indicator.
    Dots,
    /// Horizontal progress bar.
    Bar,
}

/// Loading indicator with optional status text.
#[derive(IntoElement)]
pub struct Loading {
    variant: LoadingVariant,
    size: ComponentSize,
    text: Option<SharedString>,
    fullscreen: bool,
}

impl Loading {
    /// Creates a spinner.
    pub fn new() -> Self {
        Self {
            variant: LoadingVariant::Spinner,
            size: ComponentSize::Medium,
            text: None,
            fullscreen: false,
        }
    }

    /// Sets indicator style.
    pub fn variant(mut self, variant: LoadingVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets size.
    pub fn size(mut self, size: ComponentSize) -> Self {
        self.size = size;
        self
    }

    /// Adds status text.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Makes the indicator cover its containing block.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }
}

impl Default for Loading {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Loading {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let glyph = match self.variant {
            LoadingVariant::Spinner => "◌",
            LoadingVariant::Dots => "•••",
            LoadingVariant::Bar => "━",
        };
        let mut indicator = div()
            .id("tdesign-loading")
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(gpui::rgb(0x0052d9))
            .text_size(self.size.height())
            .child(glyph);
        if let Some(text) = self.text {
            indicator = indicator.child(
                div()
                    .text_base()
                    .text_color(gpui::rgb(0x666666))
                    .child(text),
            );
        }
        if self.fullscreen {
            div()
                .id("tdesign-loading-overlay")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0xffffffcc))
                .child(indicator)
        } else {
            indicator
        }
    }
}

/// Loading component module.
pub mod loading {
    pub use super::{Loading, LoadingVariant};
}

/// Progress status color.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProgressStatus {
    /// Normal brand progress.
    #[default]
    Default,
    /// Completed successfully.
    Success,
    /// Warning state.
    Warning,
    /// Error state.
    Error,
}

/// Linear or circular progress display.
#[derive(IntoElement)]
pub struct Progress {
    percentage: f32,
    status: ProgressStatus,
    circular: bool,
    show_label: bool,
    stroke_width: gpui::Pixels,
}

impl Progress {
    /// Creates progress with a percentage clamped to 0..=100.
    pub fn new(percentage: f32) -> Self {
        Self {
            percentage: percentage.clamp(0., 100.),
            status: ProgressStatus::Default,
            circular: false,
            show_label: true,
            stroke_width: px(6.),
        }
    }

    /// Sets status.
    pub fn status(mut self, status: ProgressStatus) -> Self {
        self.status = status;
        self
    }

    /// Uses a circular presentation.
    pub fn circular(mut self, circular: bool) -> Self {
        self.circular = circular;
        self
    }

    /// Controls percentage label.
    pub fn show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    /// Sets line thickness.
    pub fn stroke_width(mut self, stroke_width: impl Into<gpui::Pixels>) -> Self {
        self.stroke_width = stroke_width.into();
        self
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color: Hsla = match self.status {
            ProgressStatus::Default => gpui::rgb(0x0052d9).into(),
            ProgressStatus::Success => gpui::rgb(0x2ba471).into(),
            ProgressStatus::Warning => gpui::rgb(0xe37318).into(),
            ProgressStatus::Error => gpui::rgb(0xd54941).into(),
        };
        if self.circular {
            div()
                .id("tdesign-progress-circle")
                .size_16()
                .rounded_full()
                .border(self.stroke_width)
                .border_color(gpui::rgb(0xe7e7e7))
                .child(
                    div()
                        .size_full()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(color)
                        .child(if self.show_label {
                            format!("{:.0}%", self.percentage)
                        } else {
                            String::new()
                        }),
                )
        } else {
            div()
                .id("tdesign-progress")
                .flex()
                .items_center()
                .gap_2()
                .w_full()
                .child(
                    div()
                        .id("tdesign-progress-track")
                        .flex_1()
                        .h(self.stroke_width)
                        .rounded_full()
                        .bg(gpui::rgb(0xe7e7e7))
                        .child(
                            div()
                                .h_full()
                                .w(gpui::relative(self.percentage / 100.))
                                .rounded_full()
                                .bg(color),
                        ),
                )
                .when(self.show_label, |this| {
                    this.child(format!("{:.0}%", self.percentage))
                })
        }
    }
}

/// Progress component module.
pub mod progress {
    pub use super::{Progress, ProgressStatus};
}

/// QR code rendered as a native vector module grid.
#[derive(IntoElement)]
pub struct QRCode {
    value: SharedString,
    size: gpui::Pixels,
    foreground: Hsla,
    background: Hsla,
    level: QrCodeEcc,
}

impl QRCode {
    /// Creates a QR code.
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            size: px(160.),
            foreground: gpui::black().into(),
            background: gpui::white().into(),
            level: QrCodeEcc::Medium,
        }
    }

    /// Sets rendered size.
    pub fn size(mut self, size: impl Into<gpui::Pixels>) -> Self {
        self.size = size.into();
        self
    }

    /// Sets foreground color.
    pub fn foreground(mut self, color: impl Into<Hsla>) -> Self {
        self.foreground = color.into();
        self
    }

    /// Sets background color.
    pub fn background(mut self, color: impl Into<Hsla>) -> Self {
        self.background = color.into();
        self
    }

    /// Sets error-correction level.
    pub fn error_correction(mut self, level: QrCodeEcc) -> Self {
        self.level = level;
        self
    }
}

impl RenderOnce for QRCode {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let code = QrCode::encode_text(&self.value, self.level).unwrap_or_else(|_| {
            QrCode::encode_text("TDesign", QrCodeEcc::Low).expect("fallback QR")
        });
        let module_count = code.size() as usize;
        let module = self.size / module_count as f32;
        let mut cells = Vec::new();
        for y in 0..module_count {
            for x in 0..module_count {
                if code.get_module(x as i32, y as i32) {
                    cells.push(
                        div()
                            .absolute()
                            .left(module * x as f32)
                            .top(module * y as f32)
                            .w(module)
                            .h(module)
                            .bg(self.foreground)
                            .into_any_element(),
                    );
                }
            }
        }
        div()
            .id("tdesign-qrcode")
            .relative()
            .w(self.size)
            .h(self.size)
            .bg(self.background)
            .children(cells)
    }
}

/// QRCode component module.
pub mod qr_code {
    pub use super::QRCode;
    pub use qrcodegen::QrCodeEcc;
}

/// Placeholder skeleton display.
#[derive(IntoElement)]
pub struct Skeleton {
    rows: usize,
    avatar: bool,
    animated: bool,
    width: gpui::Pixels,
}

impl Skeleton {
    /// Creates a three-row skeleton.
    pub fn new() -> Self {
        Self {
            rows: 3,
            avatar: false,
            animated: true,
            width: px(280.),
        }
    }

    /// Sets row count.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Adds an avatar placeholder.
    pub fn avatar(mut self, avatar: bool) -> Self {
        self.avatar = avatar;
        self
    }

    /// Controls shimmer-style opacity.
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    /// Sets maximum width.
    pub fn width(mut self, width: impl Into<gpui::Pixels>) -> Self {
        self.width = width.into();
        self
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Skeleton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let bars = (0..self.rows)
            .map(|index| {
                div()
                    .id(SharedString::from(format!("skeleton-row-{index}")))
                    .h(px(14.))
                    .w(if index + 1 == self.rows {
                        self.width * 0.62
                    } else {
                        self.width
                    })
                    .rounded_sm()
                    .bg(gpui::rgb(0xe7e7e7))
                    .when(self.animated, |this| this.opacity(0.7))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id("tdesign-skeleton")
            .flex()
            .items_start()
            .gap_3()
            .w(self.width)
            .children(
                self.avatar
                    .then(|| div().size_10().rounded_full().bg(gpui::rgb(0xe7e7e7))),
            )
            .child(div().flex().flex_col().gap_2().children(bars))
    }
}

/// Skeleton component module.
pub mod skeleton {
    pub use super::Skeleton;
}

/// Statistic trend direction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatisticTrend {
    /// No trend marker.
    #[default]
    None,
    /// Increasing value.
    Up,
    /// Decreasing value.
    Down,
}

/// Numeric statistic with prefix, suffix and trend.
#[derive(IntoElement)]
pub struct Statistic {
    title: SharedString,
    value: SharedString,
    prefix: Option<SharedString>,
    suffix: Option<SharedString>,
    trend: StatisticTrend,
}

impl Statistic {
    /// Creates a statistic.
    pub fn new(title: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
            prefix: None,
            suffix: None,
            trend: StatisticTrend::None,
        }
    }

    /// Sets prefix.
    pub fn prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Sets suffix.
    pub fn suffix(mut self, suffix: impl Into<SharedString>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// Sets trend.
    pub fn trend(mut self, trend: StatisticTrend) -> Self {
        self.trend = trend;
        self
    }
}

impl RenderOnce for Statistic {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let trend = match self.trend {
            StatisticTrend::None => "",
            StatisticTrend::Up => "↑",
            StatisticTrend::Down => "↓",
        };
        div()
            .id("tdesign-statistic")
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgb(0x777777))
                    .child(self.title),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_1()
                    .child(self.prefix.unwrap_or_default())
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(self.value),
                    )
                    .child(self.suffix.unwrap_or_default())
                    .child(
                        div()
                            .text_color(match self.trend {
                                StatisticTrend::Up => gpui::rgb(0x2ba471),
                                StatisticTrend::Down => gpui::rgb(0xd54941),
                                StatisticTrend::None => gpui::rgb(0x777777),
                            })
                            .child(trend),
                    ),
            )
    }
}

/// Statistic component module.
pub mod statistic {
    pub use super::{Statistic, StatisticTrend};
}

/// Carousel index state.
#[derive(Debug)]
pub struct SwiperState {
    /// Current slide index.
    pub index: usize,
    /// Number of slides.
    pub count: usize,
    /// Native focus handle.
    pub focus_handle: FocusHandle,
}

impl SwiperState {
    /// Creates carousel state.
    pub fn new(cx: &mut App, count: usize) -> Entity<Self> {
        cx.new(|cx| Self {
            index: 0,
            count,
            focus_handle: cx.focus_handle(),
        })
    }

    /// Moves by a signed slide offset.
    pub fn shift(&mut self, amount: isize, cx: &mut Context<Self>) {
        if self.count == 0 {
            return;
        }
        self.index = (self.index as isize + amount).rem_euclid(self.count as isize) as usize;
        cx.notify();
    }
}

/// Carousel with native controls and indicator dots.
#[derive(IntoElement)]
pub struct Swiper {
    state: Entity<SwiperState>,
    items: Vec<AnyElement>,
    show_arrows: bool,
}

impl Swiper {
    /// Creates a swiper.
    pub fn new(
        state: Entity<SwiperState>,
        items: impl IntoIterator<Item = impl IntoElement>,
    ) -> Self {
        Self {
            state,
            items: items
                .into_iter()
                .map(IntoElement::into_any_element)
                .collect(),
            show_arrows: true,
        }
    }

    /// Controls arrow visibility.
    pub fn show_arrows(mut self, show_arrows: bool) -> Self {
        self.show_arrows = show_arrows;
        self
    }
}

impl RenderOnce for Swiper {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let index = state.index;
        let focus = state.focus_handle.clone();
        let count = self.items.len();
        let current = self.items.into_iter().nth(index);
        let previous = self.state.clone();
        let next = self.state.clone();
        let dots = (0..count)
            .map(|dot| {
                div()
                    .id(SharedString::from(format!("swiper-dot-{dot}")))
                    .size_2()
                    .rounded_full()
                    .bg(if dot == index {
                        gpui::rgb(0x0052d9)
                    } else {
                        gpui::rgb(0xbdbdbd)
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        div()
            .id(entity_id("tdesign-swiper", &self.state))
            .track_focus(&focus)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .min_h(px(120.))
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_2()
                    .id("swiper-previous")
                    .when(!self.show_arrows, |this| this.invisible())
                    .child("‹")
                    .on_click(move |_, _, cx| {
                        let _ = previous.update(cx, |state, cx| state.shift(-1, cx));
                    }),
            )
            .children(current)
            .child(
                div()
                    .absolute()
                    .right_2()
                    .id("swiper-next")
                    .when(!self.show_arrows, |this| this.invisible())
                    .child("›")
                    .on_click(move |_, _, cx| {
                        let _ = next.update(cx, |state, cx| state.shift(1, cx));
                    }),
            )
            .child(div().absolute().bottom_2().flex().gap_1().children(dots))
    }
}

/// Swiper component module.
pub mod swiper {
    pub use super::{Swiper, SwiperState};
}

/// Tag semantic theme.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TagTheme {
    /// Neutral tag.
    #[default]
    Default,
    /// Brand tag.
    Primary,
    /// Success tag.
    Success,
    /// Warning tag.
    Warning,
    /// Error tag.
    Danger,
}

/// Compact semantic label with optional close action.
#[derive(IntoElement)]
pub struct Tag {
    label: SharedString,
    theme: TagTheme,
    closable: bool,
    disabled: bool,
    on_close: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
}

impl Tag {
    /// Creates a tag.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            theme: TagTheme::Default,
            closable: false,
            disabled: false,
            on_close: None,
        }
    }

    /// Sets semantic theme.
    pub fn theme(mut self, theme: TagTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Enables close affordance.
    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// Sets disabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Registers close action.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Arc::new(handler));
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (background, foreground): (Hsla, Hsla) = match self.theme {
            TagTheme::Default => (gpui::rgb(0xf3f3f3).into(), gpui::rgb(0x555555).into()),
            TagTheme::Primary => (gpui::rgb(0xe8f1ff).into(), gpui::rgb(0x0052d9).into()),
            TagTheme::Success => (gpui::rgb(0xe8f8f0).into(), gpui::rgb(0x2ba471).into()),
            TagTheme::Warning => (gpui::rgb(0xfff1e9).into(), gpui::rgb(0xe37318).into()),
            TagTheme::Danger => (gpui::rgb(0xfff0ed).into(), gpui::rgb(0xd54941).into()),
        };
        let close = self.on_close;
        div()
            .id(SharedString::from(format!("tdesign-tag-{}", self.label)))
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(background)
            .text_color(foreground)
            .when(self.disabled, |this| this.opacity(0.5))
            .child(self.label)
            .when(self.closable, |this| {
                this.child(
                    div()
                        .id("tdesign-tag-close")
                        .child("×")
                        .when(!self.disabled, |this| {
                            this.on_click(move |_, window, cx| {
                                if let Some(close) = &close {
                                    close(window, cx);
                                }
                            })
                        }),
                )
            })
    }
}

/// Tag component module.
pub mod tag {
    pub use super::{Tag, TagTheme};
}
