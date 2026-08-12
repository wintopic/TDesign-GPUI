//! TDesign's desktop component model implemented with native GPUI elements.
//!
//! This is an independent community implementation. TDesign and GPUI remain
//! trademarks and copyrighted projects of their respective authors.
//!
//! Start an application with [`gpui::Application`], install
//! [`TDesignAssetSource`], call [`init`], and render application content inside
//! [`TDesignRoot`]. Stateful controls use GPUI [`gpui::Entity`] values and typed
//! events; the default feature embeds the complete TDesign icon set.
//!
//! ```no_run
//! use gpui::{App, Application, Context, Render, Window, WindowOptions, div, prelude::*};
//! use tdesign_gpui::{Button, ButtonVariant, TDesignAssetSource, TDesignRoot};
//!
//! struct Example;
//!
//! impl Render for Example {
//!     fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
//!         TDesignRoot::new().child(
//!             div().p_4().child(
//!                 Button::new("Hello TDesign").variant(ButtonVariant::Primary),
//!             ),
//!         )
//!     }
//! }
//!
//! Application::new()
//!     .with_assets(TDesignAssetSource::new())
//!     .run(|cx: &mut App| {
//!         tdesign_gpui::init(cx);
//!         cx.open_window(WindowOptions::default(), |_window, cx| cx.new(|_| Example))
//!             .expect("open example window");
//!         cx.activate(true);
//!     });
//! ```

mod components;
mod config;
mod data;
mod display_a;
mod display_b;
mod display_c;
mod feedback;
mod forms;
mod foundations;
mod generated_theme_tokens;
mod interactive;
mod locale;
mod navigation;
mod overlay;
pub mod parity;
mod root;
mod state;
mod theme;
mod utility;

pub use components::*;
pub use config::{TDesignConfig, TDesignConfigState};
pub use data::*;
pub use display_a::*;
pub use display_b::*;
pub use display_c::*;
pub use feedback::*;
pub use forms::*;
pub use foundations::*;
pub use generated_theme_tokens::{TDESIGN_WEB_TOKENS, UpstreamThemeToken};
pub use interactive::*;
pub use locale::text as locale_text;
pub use locale::{Locale, LocaleMessages, TDesignLocaleGlobal};
pub use navigation::*;
pub use overlay::{OverlayId, OverlayKind, OverlayState};
pub use root::TDesignRoot;
pub use state::{
    HttpUploadBackend, InputEvent, InputState, ListDelegate, TableDelegate, TreeNode,
    UploadBackend, UploadCancellation, UploadProgress, UploadRequest, ValueChange,
};
pub use tdesign_gpui_assets as assets;
pub use tdesign_gpui_assets::{IconName, TDesignAssetSource, UnknownIcon};
pub use theme::{TDesignTheme, TDesignThemeGlobal, ThemeMode, ThemeOverrides, ThemeTokens};
pub use utility::*;

/// A named or anonymous child element accepted by component slots.
pub type TElement = gpui::AnyElement;
/// Alias used by the TDesign migration guide for a child node.
pub type TNode = gpui::AnyElement;

/// Installs default TDesign globals in a GPUI application.
pub fn init(app: &mut gpui::App) {
    if !app.has_global::<TDesignThemeGlobal>() {
        app.set_global(TDesignThemeGlobal(TDesignTheme::light()));
    }
    if !app.has_global::<TDesignLocaleGlobal>() {
        app.set_global(TDesignLocaleGlobal {
            locale: Locale::default(),
            messages: LocaleMessages::default(),
        });
    }
}

/// Common imports for TDesign GPUI applications.
pub mod prelude {
    pub use crate::{
        Button, ButtonVariant, ComponentSize, TDesignTheme, ThemeMode, ThemeOverrides,
    };
    pub use crate::{Icon, IconName, TDesignAssetSource, TDesignConfig, TDesignRoot, init};
    pub use gpui::prelude::*;
}
