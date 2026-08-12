//! Localized strings and locale selection.

use gpui::Global;
use std::collections::BTreeMap;

/// A locale identifier supported by TDesign.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Locale(String);
impl Locale {
    /// Simplified Chinese, the default locale.
    pub fn zh_cn() -> Self {
        Self("zh-CN".into())
    }
    /// English locale.
    pub fn en_us() -> Self {
        Self("en-US".into())
    }
    /// Creates a locale from a BCP-47 identifier.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    /// Returns the BCP-47 identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Default for Locale {
    fn default() -> Self {
        Self::zh_cn()
    }
}
impl From<&str> for Locale {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
impl From<String> for Locale {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Component strings supplied by an application or upstream locale.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct LocaleMessages {
    /// Message values keyed by stable TDesign keys.
    pub values: BTreeMap<String, String>,
}
impl LocaleMessages {
    /// Inserts a translated string.
    pub fn insert(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }
    /// Gets a translated string.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

/// Global locale configuration.
#[derive(Clone, Debug)]
pub struct TDesignLocaleGlobal {
    /// Active locale.
    pub locale: Locale,
    /// Custom messages.
    pub messages: LocaleMessages,
}
impl Global for TDesignLocaleGlobal {}

/// Built-in copy with an English fallback for custom locales.
pub fn builtin(locale: &Locale, key: &str) -> &'static str {
    match (locale.as_str(), key) {
        ("zh-CN", "loading") => "加载中",
        ("zh-CN", "empty") => "暂无数据",
        ("zh-CN", "confirm") => "确定",
        ("zh-CN", "cancel") => "取消",
        ("en-US", "loading") => "Loading",
        ("en-US", "empty") => "No data",
        ("en-US", "confirm") => "Confirm",
        ("en-US", "cancel") => "Cancel",
        (_, "loading") => "Loading",
        (_, "empty") => "No data",
        (_, "confirm") => "Confirm",
        (_, "cancel") => "Cancel",
        _ => "",
    }
}
