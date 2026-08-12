//! Localized strings and locale selection.

use gpui::{App, Global, SharedString};
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
pub fn builtin<'a>(locale: &Locale, key: &'a str) -> &'a str {
    match (locale.as_str(), key) {
        ("zh-CN", "loading") => "加载中",
        ("zh-CN", "empty") => "暂无数据",
        ("zh-CN", "confirm") => "确定",
        ("zh-CN", "cancel") => "取消",
        ("zh-CN", "select-date") => "选择日期",
        ("zh-CN", "select-time") => "选择时间",
        ("zh-CN", "input-placeholder") => "请输入",
        ("zh-CN", "select-placeholder") => "请选择",
        ("zh-CN", "tag-input-placeholder") => "请输入并确认",
        ("zh-CN", "transfer-source") => "源列表",
        ("zh-CN", "transfer-target") => "目标列表",
        ("zh-CN", "choose-file") => "选择文件",
        ("zh-CN", "upload-ready") => "待上传",
        ("zh-CN", "upload-uploading") => "上传中",
        ("zh-CN", "upload-success") => "已完成",
        ("zh-CN", "upload-failed") => "失败",
        ("zh-CN", "upload-canceled") => "已取消",
        ("zh-CN", "retry") => "重试",
        ("en-US", "loading") => "Loading",
        ("en-US", "empty") => "No data",
        ("en-US", "confirm") => "Confirm",
        ("en-US", "cancel") => "Cancel",
        ("en-US", "select-date") => "Select date",
        ("en-US", "select-time") => "Select time",
        ("en-US", "input-placeholder") => "Enter a value",
        ("en-US", "select-placeholder") => "Select an option",
        ("en-US", "tag-input-placeholder") => "Enter and press Enter",
        ("en-US", "transfer-source") => "Source",
        ("en-US", "transfer-target") => "Target",
        ("en-US", "choose-file") => "Choose file",
        ("en-US", "upload-ready") => "Ready",
        ("en-US", "upload-uploading") => "Uploading",
        ("en-US", "upload-success") => "Completed",
        ("en-US", "upload-failed") => "Failed",
        ("en-US", "upload-canceled") => "Canceled",
        ("en-US", "retry") => "Retry",
        (_, "loading") => "Loading",
        (_, "empty") => "No data",
        (_, "confirm") => "Confirm",
        (_, "cancel") => "Cancel",
        (_, "select-date") => "Select date",
        (_, "select-time") => "Select time",
        (_, "input-placeholder") => "Enter a value",
        (_, "select-placeholder") => "Select an option",
        (_, "tag-input-placeholder") => "Enter and press Enter",
        (_, "transfer-source") => "Source",
        (_, "transfer-target") => "Target",
        (_, "choose-file") => "Choose file",
        (_, "upload-ready") => "Ready",
        (_, "upload-uploading") => "Uploading",
        (_, "upload-success") => "Completed",
        (_, "upload-failed") => "Failed",
        (_, "upload-canceled") => "Canceled",
        (_, "retry") => "Retry",
        _ => key,
    }
}

/// Resolves an application override, then built-in copy, then the stable key.
pub fn text(cx: &App, key: &'static str) -> SharedString {
    if let Some(global) = cx.try_global::<TDesignLocaleGlobal>() {
        if let Some(value) = global.messages.get(key) {
            return value.to_owned().into();
        }
        return builtin(&global.locale, key).into();
    }
    builtin(&Locale::default(), key).into()
}
