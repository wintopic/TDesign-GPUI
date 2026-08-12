//! Small compile-and-render smoke application for local component development.

use gpui::{Context, IntoElement, Render, SharedString, Window, div, prelude::*};
use tdesign_gpui::{
    Button, ButtonVariant, ComponentSize, Disableable, Icon, IconName, Sizable, TDesignRoot,
    parity::COMPONENTS,
};

/// A compact smoke view that renders representative component surfaces.
pub struct Gallery {
    title: SharedString,
}
impl Gallery {
    /// Creates the gallery.
    pub fn new() -> Self {
        Self {
            title: "TDesign GPUI smoke gallery".into(),
        }
    }
}
impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}
impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let component_badges = COMPONENTS.iter().enumerate().map(|(index, component)| {
            div()
                .id(SharedString::from(format!("gallery-component-{index}")))
                .px_2()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(gpui::rgb(0xdcdcdc))
                .child(*component)
        });
        TDesignRoot::new().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(self.title.clone())
                .child(
                    div().flex().gap_2().children([
                        Button::new("Primary")
                            .variant(ButtonVariant::Primary)
                            .size(ComponentSize::Medium)
                            .into_any_element(),
                        Button::new("Disabled").disabled(true).into_any_element(),
                        Icon::new(IconName::CheckCircleFilled).into_any_element(),
                    ]),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::rgb(0x666666))
                        .child(format!(
                            "API inventory: {} component names",
                            COMPONENTS.len()
                        )),
                )
                .child(div().flex().flex_wrap().gap_2().children(component_badges)),
        )
    }
}
