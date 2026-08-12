//! Minimal native TDesign GPUI desktop application.

use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
use tdesign_gpui::{
    Button, ButtonVariant, Icon, IconName, Input, InputState, TDesignAssetSource, TDesignRoot,
};

struct BasicExample {
    input: gpui::Entity<InputState>,
}

impl BasicExample {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            input: InputState::new(cx, ""),
        }
    }
}

impl Render for BasicExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TDesignRoot::new().child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_6()
                .child("TDesign GPUI")
                .child(Input::new(self.input.clone()))
                .child(
                    Button::new("确定")
                        .variant(ButtonVariant::Primary)
                        .icon(Icon::new(IconName::Check)),
                ),
        )
    }
}

fn main() {
    Application::new()
        .with_assets(TDesignAssetSource::new())
        .run(|cx: &mut App| {
            tdesign_gpui::init(cx);
            cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(BasicExample::new)
            })
            .expect("open TDesign GPUI example window");
            cx.activate(true);
        });
}
