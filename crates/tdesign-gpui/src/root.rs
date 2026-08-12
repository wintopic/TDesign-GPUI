use crate::{
    Locale, OverlayKind, OverlayState, TDesignConfig, TDesignConfigState, TDesignLocaleGlobal,
    TDesignThemeGlobal,
};
use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, RenderOnce, Window, div, prelude::*,
};

/// Window-level host corresponding to React `ConfigProvider`.
#[derive(IntoElement)]
pub struct TDesignRoot {
    config: TDesignConfig,
    config_state: Option<Entity<TDesignConfigState>>,
    child: Option<AnyElement>,
    overlays: Option<Entity<OverlayState>>,
}

impl TDesignRoot {
    /// Creates a root with default light theme and zh-CN locale.
    pub fn new() -> Self {
        Self {
            config: TDesignConfig::default(),
            config_state: None,
            child: None,
            overlays: None,
        }
    }
    /// Creates a root with explicit configuration.
    pub fn with_config(config: TDesignConfig) -> Self {
        Self {
            config,
            config_state: None,
            child: None,
            overlays: None,
        }
    }
    /// Replaces the root configuration.
    pub fn config(mut self, config: TDesignConfig) -> Self {
        self.config = config;
        self
    }
    /// Uses an entity-backed configuration that can be changed at runtime.
    pub fn config_state(mut self, state: Entity<TDesignConfigState>) -> Self {
        self.config_state = Some(state);
        self
    }
    /// Sets the locale without rebuilding the configuration.
    pub fn locale(mut self, locale: impl Into<Locale>) -> Self {
        self.config.locale = locale.into();
        self
    }
    /// Adds the application content.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }
    /// Attaches the window-local popup/dialog/message host.
    pub fn overlays(mut self, state: Entity<OverlayState>) -> Self {
        self.overlays = Some(state);
        self
    }
    /// Returns the current root configuration.
    pub fn configuration(&self) -> &TDesignConfig {
        &self.config
    }
}

impl Default for TDesignRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for TDesignRoot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let config = self
            .config_state
            .as_ref()
            .map(|state| state.read(cx).config.clone())
            .unwrap_or(self.config);
        let tokens = config.theme.tokens_for_window(window);
        // Keep the global theme in sync with the window-resolved palette. This
        // matters for `ThemeMode::System`: components that read the global must
        // see the same dark/light tokens as the root background.
        let mut resolved_theme = config.theme.clone();
        resolved_theme.tokens = tokens.clone();
        cx.set_global(TDesignThemeGlobal(resolved_theme));
        cx.set_global(TDesignLocaleGlobal {
            locale: config.locale,
            messages: config.messages,
        });
        let mut root = div()
            .relative()
            .size_full()
            .bg(tokens.background)
            .text_color(tokens.text)
            .children(self.child);
        if let Some(overlays) = self.overlays {
            let key_overlays = overlays.clone();
            let entries = overlays.read(cx).entries();
            let mut messages = Vec::new();
            let mut notifications = Vec::new();
            for entry in entries {
                if entry.kind == OverlayKind::Message {
                    messages.push((entry.render)(window, cx));
                    continue;
                }
                if entry.kind == OverlayKind::Notification {
                    notifications.push((entry.render)(window, cx));
                    continue;
                }
                let content = (entry.render)(window, cx);
                let layer = div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .flex()
                    .when(entry.modal, |this| this.occlude())
                    .when(entry.modal, |this| this.bg(gpui::black().opacity(0.45)));
                let layer = match entry.kind {
                    OverlayKind::Dialog | OverlayKind::Popconfirm | OverlayKind::Guide => {
                        layer.items_center().justify_center()
                    }
                    OverlayKind::Drawer => layer.justify_end(),
                    OverlayKind::Message | OverlayKind::Notification => unreachable!(),
                    OverlayKind::Popup => layer.items_start(),
                }
                .child(content);
                root = root.child(layer);
            }
            if !messages.is_empty() {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_start()
                        .justify_center()
                        .pt_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .children(messages),
                        ),
                );
            }
            if !notifications.is_empty() {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_start()
                        .justify_end()
                        .p_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_end()
                                .gap_3()
                                .children(notifications),
                        ),
                );
            }
            root = root.on_key_down(move |event, window, cx| {
                if event.keystroke.key.eq_ignore_ascii_case("escape") {
                    let _ = key_overlays.update(cx, |state, cx| state.dismiss_top(window, cx));
                }
            });
        }
        root
    }
}
