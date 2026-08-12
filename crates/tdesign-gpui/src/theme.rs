//! Theme tokens and runtime appearance configuration.

use gpui::{Global, Hsla, WindowAppearance, rgb};
use std::collections::BTreeMap;

use crate::generated_theme_tokens::TDESIGN_WEB_TOKENS;

/// The requested color-scheme policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ThemeMode {
    /// TDesign's light palette.
    #[default]
    Light,
    /// TDesign's dark palette.
    Dark,
    /// Follow the host window appearance.
    System,
}

/// Typed core tokens plus an extension map for new upstream variables.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ThemeTokens {
    /// Primary brand color.
    pub brand: Hsla,
    /// Application background.
    pub background: Hsla,
    /// Raised container background.
    pub surface: Hsla,
    /// Primary text.
    pub text: Hsla,
    /// Secondary text.
    pub text_secondary: Hsla,
    /// Divider and border color.
    pub border: Hsla,
    /// Error color.
    pub error: Hsla,
    /// Warning color.
    pub warning: Hsla,
    /// Success color.
    pub success: Hsla,
    /// Additional upstream variables keyed by their `--td-*` or Less alias.
    pub extensions: BTreeMap<String, String>,
}

impl ThemeTokens {
    /// Returns the standard TDesign light tokens.
    pub fn light() -> Self {
        Self {
            brand: rgb(0x0052d9).into(),
            background: rgb(0xf3f3f3).into(),
            surface: rgb(0xffffff).into(),
            text: rgb(0x1f1f1f).into(),
            text_secondary: rgb(0x666666).into(),
            border: rgb(0xe7e7e7).into(),
            error: rgb(0xd54941).into(),
            warning: rgb(0xe37318).into(),
            success: rgb(0x2ba471).into(),
            extensions: upstream_extensions("light"),
        }
    }

    /// Returns the standard TDesign dark tokens.
    pub fn dark() -> Self {
        Self {
            brand: rgb(0x4582e6).into(),
            background: rgb(0x181818).into(),
            surface: rgb(0x242424).into(),
            text: rgb(0xe7e7e7).into(),
            text_secondary: rgb(0xa6a6a6).into(),
            border: rgb(0x434343).into(),
            error: rgb(0xe37371).into(),
            warning: rgb(0xe9a23b).into(),
            success: rgb(0x56c596).into(),
            extensions: upstream_extensions("dark"),
        }
    }

    /// Adds or replaces an upstream token value.
    pub fn insert(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extensions.insert(name.into(), value.into());
        self
    }
}

/// User supplied token changes merged over the active palette.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ThemeOverrides {
    /// Optional brand color override.
    pub brand: Option<Hsla>,
    /// Arbitrary token overrides.
    pub tokens: BTreeMap<String, String>,
}

impl ThemeOverrides {
    /// Creates an empty override set.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the primary brand color.
    pub fn brand(mut self, color: impl Into<Hsla>) -> Self {
        self.brand = Some(color.into());
        self
    }
    /// Sets an arbitrary token.
    pub fn token(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.tokens.insert(name.into(), value.into());
        self
    }
}

/// The resolved theme used by a root.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TDesignTheme {
    /// Color-scheme policy.
    pub mode: ThemeMode,
    /// Resolved tokens.
    pub tokens: ThemeTokens,
    /// Base corner radius in pixels.
    pub radius: f32,
    /// User overrides retained when system appearance changes.
    pub overrides: ThemeOverrides,
}

impl Default for TDesignTheme {
    fn default() -> Self {
        Self::light()
    }
}
impl TDesignTheme {
    /// Creates the light theme.
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            tokens: ThemeTokens::light(),
            radius: 3.0,
            overrides: ThemeOverrides::default(),
        }
    }
    /// Creates the dark theme.
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            tokens: ThemeTokens::dark(),
            radius: 3.0,
            overrides: ThemeOverrides::default(),
        }
    }
    /// Creates a system-following theme with light fallback tokens.
    pub fn system() -> Self {
        Self {
            mode: ThemeMode::System,
            ..Self::light()
        }
    }
    /// Applies user overrides.
    pub fn with_overrides(mut self, overrides: ThemeOverrides) -> Self {
        apply_overrides(&mut self.tokens, &overrides);
        self.overrides = overrides;
        self
    }
    /// Resolves system appearance for a GPUI window.
    pub fn tokens_for_window(&self, window: &gpui::Window) -> ThemeTokens {
        let dark = match self.mode {
            ThemeMode::Dark => true,
            ThemeMode::System => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
            ThemeMode::Light => false,
        };
        if dark {
            let mut tokens = if matches!(self.mode, ThemeMode::Dark) {
                self.tokens.clone()
            } else {
                ThemeTokens::dark()
            };
            apply_overrides(&mut tokens, &self.overrides);
            tokens
        } else {
            let mut tokens = if matches!(self.mode, ThemeMode::Light) {
                self.tokens.clone()
            } else {
                ThemeTokens::light()
            };
            apply_overrides(&mut tokens, &self.overrides);
            tokens
        }
    }
}

fn upstream_extensions(mode: &str) -> BTreeMap<String, String> {
    TDESIGN_WEB_TOKENS
        .iter()
        .filter(|token| matches!(token.mode, "alias" | "common") || token.mode == mode)
        .map(|token| (token.name.to_owned(), token.value.to_owned()))
        .collect()
}

fn apply_overrides(tokens: &mut ThemeTokens, overrides: &ThemeOverrides) {
    if let Some(brand) = overrides.brand {
        tokens.brand = brand;
    }
    tokens.extensions.extend(overrides.tokens.clone());
}

/// Global theme configuration installed by [`crate::init`].
#[derive(Clone, Debug)]
pub struct TDesignThemeGlobal(pub TDesignTheme);
impl Global for TDesignThemeGlobal {}
