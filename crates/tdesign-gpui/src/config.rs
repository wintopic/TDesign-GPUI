//! Native equivalent of React's `ConfigProvider`.

use crate::{Locale, LocaleMessages, TDesignTheme, ThemeMode, ThemeOverrides};
use gpui::{App, AppContext, Context, Entity};

/// Root-level options shared by every TDesign component in a window.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TDesignConfig {
    /// Active theme.
    pub theme: TDesignTheme,
    /// Active locale.
    pub locale: Locale,
    /// Application translations.
    pub messages: LocaleMessages,
    /// Whether animations are enabled.
    pub motion: bool,
    /// Whether reduced motion is requested.
    pub reduced_motion: bool,
}
impl Default for TDesignConfig {
    fn default() -> Self {
        Self {
            theme: TDesignTheme::light(),
            locale: Locale::default(),
            messages: LocaleMessages::default(),
            motion: true,
            reduced_motion: false,
        }
    }
}
impl TDesignConfig {
    /// Creates the default Simplified Chinese/light configuration.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the theme mode.
    pub fn theme_mode(mut self, mode: ThemeMode) -> Self {
        self.theme.mode = mode;
        self
    }
    /// Applies token overrides.
    pub fn theme_overrides(mut self, overrides: ThemeOverrides) -> Self {
        self.theme = self.theme.with_overrides(overrides);
        self
    }
    /// Sets the locale.
    pub fn locale(mut self, locale: impl Into<Locale>) -> Self {
        self.locale = locale.into();
        self
    }
    /// Adds a translation.
    pub fn message(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.messages = self.messages.insert(key, value);
        self
    }
    /// Enables or disables component motion.
    pub fn motion(mut self, enabled: bool) -> Self {
        self.motion = enabled;
        self
    }
    /// Enables reduced motion semantics.
    pub fn reduced_motion(mut self, enabled: bool) -> Self {
        self.reduced_motion = enabled;
        self
    }
}

/// Mutable, entity-backed configuration for a window or application root.
///
/// This is the native counterpart of changing React's `ConfigProvider` props
/// at runtime. Updating the entity invalidates any [`crate::TDesignRoot`] that
/// consumes it, so theme and locale changes take effect on the next frame.
#[derive(Clone, Debug)]
pub struct TDesignConfigState {
    /// Current root configuration.
    pub config: TDesignConfig,
}

impl TDesignConfigState {
    /// Creates an entity with the supplied configuration.
    pub fn new(cx: &mut App, config: TDesignConfig) -> Entity<Self> {
        cx.new(|_| Self { config })
    }

    /// Creates an entity with the default light/Chinese configuration.
    pub fn default_entity(cx: &mut App) -> Entity<Self> {
        Self::new(cx, TDesignConfig::default())
    }

    /// Mutates the configuration and schedules a redraw.
    pub fn update(&mut self, update: impl FnOnce(&mut TDesignConfig), cx: &mut Context<Self>) {
        update(&mut self.config);
        cx.notify();
    }

    /// Replaces the active theme.
    pub fn set_theme(&mut self, theme: TDesignTheme, cx: &mut Context<Self>) {
        self.config.theme = theme;
        cx.notify();
    }

    /// Replaces the active locale.
    pub fn set_locale(&mut self, locale: impl Into<Locale>, cx: &mut Context<Self>) {
        self.config.locale = locale.into();
        cx.notify();
    }
}
