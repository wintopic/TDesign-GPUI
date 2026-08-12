//! Native smoke gallery for the TDesign GPUI workspace.

use gpui::{Application, prelude::*};
use tdesign_gpui::TDesignAssetSource;
mod gallery;

fn main() {
    Application::new()
        .with_assets(TDesignAssetSource::new())
        .run(|app| {
            tdesign_gpui::init(app);
            app.open_window(gpui::WindowOptions::default(), |_window, cx| {
                cx.new(|_| gallery::Gallery::new())
            })
            .expect("open TDesign GPUI smoke gallery");
            app.activate(true);
        });
}
