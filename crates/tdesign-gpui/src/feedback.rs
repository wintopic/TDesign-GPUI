//! Feedback surfaces backed by the root overlay host.

use crate::{OverlayId, OverlayKind, OverlayState};
use gpui::{
    App, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce, SharedString, Window, div,
    prelude::*, px,
};
use std::{sync::Arc, time::Duration};

type DialogHandler = Arc<dyn Fn(&mut Window, &mut App)>;

/// Semantic severity shared by alerts and messages.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FeedbackLevel {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}
impl FeedbackLevel {
    fn color(self) -> gpui::Hsla {
        match self {
            Self::Info => gpui::rgb(0x0052d9).into(),
            Self::Success => gpui::rgb(0x2ba471).into(),
            Self::Warning => gpui::rgb(0xe37318).into(),
            Self::Error => gpui::rgb(0xd54941).into(),
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Self::Info => "ⓘ",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Error => "×",
        }
    }
}

/// Inline status banner.
#[derive(Clone, IntoElement)]
pub struct Alert {
    title: SharedString,
    description: Option<SharedString>,
    level: FeedbackLevel,
    closable: bool,
}
impl Alert {
    /// Creates an informational alert.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            level: FeedbackLevel::Info,
            closable: false,
        }
    }
    /// Sets secondary copy.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }
    /// Sets semantic level.
    pub fn level(mut self, level: FeedbackLevel) -> Self {
        self.level = level;
        self
    }
    /// Shows a close affordance. Visibility state is owned by the parent.
    pub fn closable(mut self, value: bool) -> Self {
        self.closable = value;
        self
    }
}
impl RenderOnce for Alert {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = self.level.color();
        div()
            .flex()
            .items_start()
            .gap_3()
            .w_full()
            .p_3()
            .rounded_sm()
            .border_1()
            .border_color(color.opacity(0.35))
            .bg(color.opacity(0.08))
            .child(div().text_color(color).child(self.level.glyph()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .child(self.title)
                    .children(
                        self.description.map(|value| {
                            div().text_sm().text_color(gpui::rgb(0x666666)).child(value)
                        }),
                    ),
            )
            .children(self.closable.then(|| div().child("×")))
    }
}
/// Alert module.
pub mod alert {
    pub use super::{Alert, FeedbackLevel};
}

/// Modal confirmation or form surface.
#[derive(Clone, IntoElement)]
pub struct Dialog {
    title: SharedString,
    body: SharedString,
    width: gpui::Pixels,
    footer: bool,
    on_confirm: Option<DialogHandler>,
    on_cancel: Option<DialogHandler>,
}
impl Dialog {
    /// Creates a dialog.
    pub fn new(title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            width: px(480.),
            footer: true,
            on_confirm: None,
            on_cancel: None,
        }
    }
    /// Sets width.
    pub fn width(mut self, value: impl Into<gpui::Pixels>) -> Self {
        self.width = value.into();
        self
    }
    /// Shows or hides the default action footer.
    pub fn footer(mut self, value: bool) -> Self {
        self.footer = value;
        self
    }
    /// Registers the default confirm action.
    pub fn on_confirm(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Arc::new(handler));
        self
    }
    /// Registers the close and cancel action.
    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Arc::new(handler));
        self
    }
    /// Opens the dialog in a root overlay host.
    pub fn open(
        self,
        overlays: &Entity<OverlayState>,
        restore_focus: Option<FocusHandle>,
        cx: &mut App,
    ) -> OverlayId {
        let title = self.title.clone();
        let body = self.body.clone();
        let width = self.width;
        let footer = self.footer;
        let on_confirm = self.on_confirm.clone();
        let on_cancel = self.on_cancel.clone();
        let weak_overlays = overlays.downgrade();
        overlays.update(cx, |state, cx| {
            let id_slot = std::rc::Rc::new(std::cell::Cell::new(None));
            let render_id = id_slot.clone();
            let id = state.show(
                OverlayKind::Dialog,
                true,
                300,
                restore_focus,
                move |_, _| {
                    let overlay_id = render_id.get();
                    Dialog {
                        title: title.clone(),
                        body: body.clone(),
                        width,
                        footer,
                        on_confirm: on_confirm.clone(),
                        on_cancel: on_cancel.clone(),
                    }
                    .overlay(weak_overlays.clone(), overlay_id)
                    .into_any_element()
                },
                cx,
            );
            id_slot.set(Some(id));
            id
        })
    }

    fn overlay(mut self, overlays: gpui::WeakEntity<OverlayState>, id: Option<OverlayId>) -> Self {
        let cancel_overlays = overlays.clone();
        let confirm_overlays = overlays;
        let cancel_dismiss = move |window: &mut Window, cx: &mut App| {
            if let Some(id) = id {
                let _ = cancel_overlays.update(cx, |state, cx| state.dismiss(id, window, cx));
            }
        };
        let confirm_dismiss = move |window: &mut Window, cx: &mut App| {
            if let Some(id) = id {
                let _ = confirm_overlays.update(cx, |state, cx| state.dismiss(id, window, cx));
            }
        };
        let cancel = self.on_cancel.clone();
        let confirm = self.on_confirm.clone();
        self.on_cancel = Some(Arc::new(move |window, cx| {
            if let Some(handler) = &cancel {
                handler(window, cx);
            }
            cancel_dismiss(window, cx);
        }));
        self.on_confirm = Some(Arc::new(move |window, cx| {
            if let Some(handler) = &confirm {
                handler(window, cx);
            }
            confirm_dismiss(window, cx);
        }));
        self
    }
}
impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cancel_text = crate::locale::text(cx, "cancel");
        let confirm_text = crate::locale::text(cx, "confirm");
        let close = self.on_cancel.clone();
        let cancel = self.on_cancel.clone();
        let confirm = self.on_confirm.clone();
        div()
            .w(self.width)
            .rounded_md()
            .bg(gpui::white())
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .py_4()
                    .border_b_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .child(self.title)
                    .child(div().id("tdesign-dialog-close").child("×").on_click(
                        move |_, window, cx| {
                            if let Some(handler) = &close {
                                handler(window, cx);
                            }
                        },
                    )),
            )
            .child(div().p_5().child(self.body))
            .children(self.footer.then(|| {
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .child(
                        div()
                            .id("tdesign-dialog-cancel")
                            .child(cancel_text)
                            .on_click(move |_, window, cx| {
                                if let Some(handler) = &cancel {
                                    handler(window, cx);
                                }
                            }),
                    )
                    .child(
                        div()
                            .id("tdesign-dialog-confirm")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .bg(gpui::rgb(0x0052d9))
                            .text_color(gpui::white())
                            .child(confirm_text)
                            .on_click(move |_, window, cx| {
                                if let Some(handler) = &confirm {
                                    handler(window, cx);
                                }
                            }),
                    )
            }))
    }
}
/// Dialog module.
pub mod dialog {
    pub use super::Dialog;
}

/// Drawer edge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DrawerPlacement {
    Left,
    Top,
    Bottom,
    #[default]
    Right,
}

/// Edge-attached modal panel.
#[derive(Clone, IntoElement)]
pub struct Drawer {
    title: SharedString,
    body: SharedString,
    placement: DrawerPlacement,
    size: gpui::Pixels,
    on_close: Option<DialogHandler>,
}
impl Drawer {
    /// Creates a right-side drawer.
    pub fn new(title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            placement: DrawerPlacement::Right,
            size: px(360.),
            on_close: None,
        }
    }
    /// Sets edge.
    pub fn placement(mut self, value: DrawerPlacement) -> Self {
        self.placement = value;
        self
    }
    /// Sets width for horizontal drawers or height for vertical drawers.
    pub fn size(mut self, value: impl Into<gpui::Pixels>) -> Self {
        self.size = value.into();
        self
    }
    /// Registers the close action.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Arc::new(handler));
        self
    }
    /// Opens the drawer.
    pub fn open(
        self,
        overlays: &Entity<OverlayState>,
        restore_focus: Option<FocusHandle>,
        cx: &mut App,
    ) -> OverlayId {
        let title = self.title.clone();
        let body = self.body.clone();
        let placement = self.placement;
        let size = self.size;
        let on_close = self.on_close.clone();
        let weak_overlays = overlays.downgrade();
        overlays.update(cx, |state, cx| {
            let id_slot = std::rc::Rc::new(std::cell::Cell::new(None));
            let render_id = id_slot.clone();
            let id = state.show(
                OverlayKind::Drawer,
                true,
                300,
                restore_focus,
                move |_, _| {
                    let overlay_id = render_id.get();
                    let weak_overlays = weak_overlays.clone();
                    let close = on_close.clone();
                    Drawer {
                        title: title.clone(),
                        body: body.clone(),
                        placement,
                        size,
                        on_close: Some(Arc::new(move |window, cx| {
                            if let Some(handler) = &close {
                                handler(window, cx);
                            }
                            if let Some(id) = overlay_id {
                                let _ = weak_overlays
                                    .update(cx, |state, cx| state.dismiss(id, window, cx));
                            }
                        })),
                    }
                    .into_any_element()
                },
                cx,
            );
            id_slot.set(Some(id));
            id
        })
    }
}
impl RenderOnce for Drawer {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let close = self.on_close.clone();
        div()
            .when(
                matches!(
                    self.placement,
                    DrawerPlacement::Left | DrawerPlacement::Right
                ),
                |this| this.w(self.size).h_full(),
            )
            .when(
                matches!(
                    self.placement,
                    DrawerPlacement::Top | DrawerPlacement::Bottom
                ),
                |this| this.h(self.size).w_full(),
            )
            .bg(gpui::white())
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_5()
                    .py_4()
                    .border_b_1()
                    .border_color(gpui::rgb(0xe7e7e7))
                    .child(self.title)
                    .child(div().id("tdesign-drawer-close").child("×").on_click(
                        move |_, window, cx| {
                            if let Some(handler) = &close {
                                handler(window, cx);
                            }
                        },
                    )),
            )
            .child(div().p_5().child(self.body))
    }
}
/// Drawer module.
pub mod drawer {
    pub use super::{Drawer, DrawerPlacement};
}

/// Contextual popup panel.
#[derive(Clone, IntoElement)]
pub struct Popup {
    content: SharedString,
    width: gpui::Pixels,
}
impl Popup {
    /// Creates a popup.
    pub fn new(content: impl Into<SharedString>) -> Self {
        Self {
            content: content.into(),
            width: px(240.),
        }
    }
    /// Sets width.
    pub fn width(mut self, value: impl Into<gpui::Pixels>) -> Self {
        self.width = value.into();
        self
    }
}
impl RenderOnce for Popup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .w(self.width)
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .bg(gpui::white())
            .shadow_md()
            .child(self.content)
    }
}
/// Popup module.
pub mod popup {
    pub use super::Popup;
}

/// Compact transient status message.
#[derive(Clone, IntoElement)]
pub struct Message {
    text: SharedString,
    level: FeedbackLevel,
    on_close: Option<Arc<dyn Fn(&mut App)>>,
}
impl Message {
    /// Creates a message.
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            level: FeedbackLevel::Info,
            on_close: None,
        }
    }
    /// Sets semantic level.
    pub fn level(mut self, level: FeedbackLevel) -> Self {
        self.level = level;
        self
    }
    /// Adds it to the message layer.
    pub fn show(self, overlays: &Entity<OverlayState>, cx: &mut App) -> OverlayId {
        let text = self.text.clone();
        let level = self.level;
        let weak_overlays = overlays.downgrade();
        let id = overlays.update(cx, |state, cx| {
            let id_slot = std::rc::Rc::new(std::cell::Cell::new(None));
            let render_id = id_slot.clone();
            let id = state.show(
                OverlayKind::Message,
                false,
                500,
                None,
                move |_, _| {
                    let overlay_id = render_id.get();
                    let weak_overlays = weak_overlays.clone();
                    Message {
                        text: text.clone(),
                        level,
                        on_close: Some(Arc::new(move |cx| {
                            if let Some(id) = overlay_id {
                                let _ = weak_overlays.update(cx, |state, cx| state.remove(id, cx));
                            }
                        })),
                    }
                    .into_any_element()
                },
                cx,
            );
            id_slot.set(Some(id));
            id
        });
        schedule_dismiss(overlays, id, cx);
        id
    }
}
impl RenderOnce for Message {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let close = self.on_close;
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(gpui::white())
            .shadow_md()
            .child(
                div()
                    .text_color(self.level.color())
                    .child(self.level.glyph()),
            )
            .child(self.text)
            .children(close.map(|close| {
                div()
                    .id("tdesign-message-close")
                    .child("×")
                    .on_click(move |_, _, cx| close(cx))
            }))
    }
}
/// Message module.
pub mod message {
    pub use super::{FeedbackLevel, Message};
}

/// Corner notification card.
#[derive(Clone, IntoElement)]
pub struct Notification {
    title: SharedString,
    body: SharedString,
    level: FeedbackLevel,
    on_close: Option<Arc<dyn Fn(&mut App)>>,
}
impl Notification {
    /// Creates a notification.
    pub fn new(title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            level: FeedbackLevel::Info,
            on_close: None,
        }
    }
    /// Sets level.
    pub fn level(mut self, level: FeedbackLevel) -> Self {
        self.level = level;
        self
    }
    /// Adds it to the notification layer.
    pub fn show(self, overlays: &Entity<OverlayState>, cx: &mut App) -> OverlayId {
        let title = self.title.clone();
        let body = self.body.clone();
        let level = self.level;
        let weak_overlays = overlays.downgrade();
        let id = overlays.update(cx, |state, cx| {
            let id_slot = std::rc::Rc::new(std::cell::Cell::new(None));
            let render_id = id_slot.clone();
            let id = state.show(
                OverlayKind::Notification,
                false,
                450,
                None,
                move |_, _| {
                    let overlay_id = render_id.get();
                    let weak_overlays = weak_overlays.clone();
                    Notification {
                        title: title.clone(),
                        body: body.clone(),
                        level,
                        on_close: Some(Arc::new(move |cx| {
                            if let Some(id) = overlay_id {
                                let _ = weak_overlays.update(cx, |state, cx| state.remove(id, cx));
                            }
                        })),
                    }
                    .into_any_element()
                },
                cx,
            );
            id_slot.set(Some(id));
            id
        });
        schedule_dismiss(overlays, id, cx);
        id
    }
}
impl RenderOnce for Notification {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let close = self.on_close;
        div()
            .w(px(360.))
            .flex()
            .gap_3()
            .p_4()
            .rounded_md()
            .bg(gpui::white())
            .shadow_lg()
            .child(
                div()
                    .text_color(self.level.color())
                    .child(self.level.glyph()),
            )
            .child(
                div().flex().flex_col().gap_1().child(self.title).child(
                    div()
                        .text_sm()
                        .text_color(gpui::rgb(0x666666))
                        .child(self.body),
                ),
            )
            .children(close.map(|close| {
                div()
                    .id("tdesign-notification-close")
                    .child("×")
                    .on_click(move |_, _, cx| close(cx))
            }))
    }
}
/// Notification module.
pub mod notification {
    pub use super::{FeedbackLevel, Notification};
}

/// Confirmation popup content.
#[derive(Clone, IntoElement)]
pub struct Popconfirm {
    message: SharedString,
    on_confirm: Option<DialogHandler>,
    on_cancel: Option<DialogHandler>,
}
impl Popconfirm {
    /// Creates a confirmation prompt.
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            on_confirm: None,
            on_cancel: None,
        }
    }
    /// Registers the confirm action.
    pub fn on_confirm(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_confirm = Some(Arc::new(handler));
        self
    }
    /// Registers the cancel action.
    pub fn on_cancel(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Arc::new(handler));
        self
    }
}
impl RenderOnce for Popconfirm {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let cancel_text = crate::locale::text(cx, "cancel");
        let confirm_text = crate::locale::text(cx, "confirm");
        let cancel = self.on_cancel;
        let confirm = self.on_confirm;
        div()
            .w(px(280.))
            .p_4()
            .rounded_md()
            .bg(gpui::white())
            .shadow_lg()
            .child(self.message)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_3()
                    .child(
                        div()
                            .id("tdesign-popconfirm-cancel")
                            .child(cancel_text)
                            .on_click(move |_, window, cx| {
                                if let Some(handler) = &cancel {
                                    handler(window, cx);
                                }
                            }),
                    )
                    .child(
                        div()
                            .id("tdesign-popconfirm-confirm")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .bg(gpui::rgb(0x0052d9))
                            .text_color(gpui::white())
                            .child(confirm_text)
                            .on_click(move |_, window, cx| {
                                if let Some(handler) = &confirm {
                                    handler(window, cx);
                                }
                            }),
                    ),
            )
    }
}

fn schedule_dismiss(overlays: &Entity<OverlayState>, id: OverlayId, cx: &mut App) {
    let weak = overlays.downgrade();
    cx.spawn(async move |async_cx| {
        async_cx
            .background_executor()
            .timer(Duration::from_secs(3))
            .await;
        let _ = async_cx.update(|cx| {
            if let Some(overlays) = weak.upgrade() {
                let _ = overlays.update(cx, |state, cx| state.remove(id, cx));
            }
        });
    })
    .detach();
}
/// Popconfirm module.
pub mod popconfirm {
    pub use super::Popconfirm;
}

/// Guided-tour step surface.
#[derive(Clone, Debug, IntoElement)]
pub struct Guide {
    title: SharedString,
    body: SharedString,
    step: usize,
    total: usize,
}
impl Guide {
    /// Creates a guide step.
    pub fn new(title: impl Into<SharedString>, body: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            step: 1,
            total: 1,
        }
    }
    /// Sets progress.
    pub fn progress(mut self, step: usize, total: usize) -> Self {
        self.step = step.max(1);
        self.total = total.max(self.step);
        self
    }
}
impl RenderOnce for Guide {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .w(px(360.))
            .p_4()
            .rounded_md()
            .bg(gpui::white())
            .shadow_lg()
            .child(self.title)
            .child(
                div()
                    .py_3()
                    .text_color(gpui::rgb(0x666666))
                    .child(self.body),
            )
            .child(format!("{} / {}", self.step, self.total))
    }
}
/// Guide module.
pub mod guide {
    pub use super::Guide;
}
