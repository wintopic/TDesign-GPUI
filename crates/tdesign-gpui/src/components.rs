//! Component implementations and compatibility modules.
#![allow(missing_docs)]

use crate::{IconName, TDesignThemeGlobal};
use gpui::{
    App, ClickEvent, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Window, div,
    prelude::*, px, svg,
};
use std::sync::Arc;

/// Standard component sizes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ComponentSize {
    Small,
    #[default]
    Medium,
    Large,
}
impl ComponentSize {
    pub(crate) fn height(self) -> gpui::Pixels {
        match self {
            Self::Small => px(24.),
            Self::Medium => px(32.),
            Self::Large => px(40.),
        }
    }
}

/// Shared enabled/disabled behavior.
pub trait Disableable: Sized {
    /// Sets disabled state.
    fn disabled(self, value: bool) -> Self;
}
/// Shared sizing behavior.
pub trait Sizable: Sized {
    /// Sets component size.
    fn size(self, value: ComponentSize) -> Self;
}

/// An icon rendered from the embedded TDesign SVG set.
#[derive(Clone, Debug, IntoElement)]
pub struct Icon {
    name: IconName,
    size: gpui::Pixels,
    color: Option<Hsla>,
}
impl Icon {
    /// Creates an icon.
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: px(16.),
            color: None,
        }
    }
    /// Sets icon dimensions.
    pub fn size(mut self, size: impl Into<gpui::Pixels>) -> Self {
        self.size = size.into();
        self
    }
    /// Sets icon tint.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }
    /// Returns the icon name.
    pub fn name(&self) -> IconName {
        self.name
    }
}
impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut element = svg().path(self.name.asset_path()).w(self.size).h(self.size);
        let color = self.color.or_else(|| {
            cx.try_global::<TDesignThemeGlobal>()
                .map(|theme| theme.0.tokens.text)
        });
        if let Some(color) = color {
            element = element.text_color(color);
        }
        element
    }
}

/// Icon component module.
pub mod icon {
    pub use super::Icon;
    pub use crate::IconName;
}

/// Button visual variants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Base,
    Primary,
    Success,
    Warning,
    Danger,
    Text,
    Outline,
}

/// A keyboard-focusable action button.
#[derive(IntoElement)]
pub struct Button {
    label: SharedString,
    variant: ButtonVariant,
    size: ComponentSize,
    disabled: bool,
    icon: Option<Icon>,
    on_click: Option<Arc<dyn Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync>>,
}
impl Button {
    /// Creates a button with text.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::Base,
            size: ComponentSize::Medium,
            disabled: false,
            icon: None,
            on_click: None,
        }
    }
    /// Sets visual variant.
    pub fn variant(mut self, value: ButtonVariant) -> Self {
        self.variant = value;
        self
    }
    /// Adds a leading icon.
    pub fn icon(mut self, value: Icon) -> Self {
        self.icon = Some(value);
        self
    }
    /// Sets click handler.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + Send + Sync + 'static,
    ) -> Self {
        self.on_click = Some(Arc::new(handler));
        self
    }
    /// Returns the label.
    pub fn label(&self) -> &str {
        &self.label
    }
}
impl Disableable for Button {
    fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
}
impl Sizable for Button {
    fn size(mut self, value: ComponentSize) -> Self {
        self.size = value;
        self
    }
}
impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (bg, fg): (Hsla, Hsla) = match self.variant {
            ButtonVariant::Primary => (gpui::rgb(0x0052d9).into(), gpui::white()),
            ButtonVariant::Success => (gpui::rgb(0x2ba471).into(), gpui::white()),
            ButtonVariant::Warning => (gpui::rgb(0xe37318).into(), gpui::white()),
            ButtonVariant::Danger => (gpui::rgb(0xd54941).into(), gpui::white()),
            ButtonVariant::Text => (gpui::transparent_black(), gpui::rgb(0x0052d9).into()),
            ButtonVariant::Outline => (gpui::white(), gpui::rgb(0x0052d9).into()),
            ButtonVariant::Base => (gpui::rgb(0xf3f3f3).into(), gpui::rgb(0x1f1f1f).into()),
        };
        let label = self.label.clone();
        let disabled = self.disabled;
        let id: SharedString = format!("tdesign-button-{}", self.label).into();
        let brand = cx
            .try_global::<TDesignThemeGlobal>()
            .map(|theme| theme.0.tokens.brand)
            .unwrap_or_else(|| gpui::rgb(0x0052d9).into());
        let mut button = div()
            .id(id)
            .focusable()
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .h(self.size.height())
            .px_4()
            .rounded_sm()
            .bg(bg)
            .text_color(fg)
            .when(self.variant == ButtonVariant::Outline, |this| {
                this.border_1().border_color(brand)
            })
            .when(!disabled, |this| this.hover(|this| this.opacity(0.85)))
            .when(disabled, |this| this.opacity(0.5));
        if let Some(icon) = self.icon {
            button = button.child(icon);
        }
        button = button.child(label);
        if let Some(handler) = self.on_click {
            button = button.on_click(move |event, window, cx| {
                if !disabled {
                    handler(event, window, cx);
                }
            });
        }
        button
    }
}
/// Button component module.
pub mod button {
    pub use super::{Button, ButtonVariant};
}

/// Native `ConfigProvider` compatibility module.
pub mod config_provider {
    pub use crate::{TDesignConfig, TDesignRoot};
}
