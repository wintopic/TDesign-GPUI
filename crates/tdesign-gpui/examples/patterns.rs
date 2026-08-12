//! Compile-checked patterns for configuration, events, uploads, and virtualized data.

use gpui::{
    AnyElement, App, Application, Context, Entity, Render, SharedString, Task, Window,
    WindowOptions, div, prelude::*,
};
use std::sync::Arc;
use tdesign_gpui::{
    Input, InputEvent, InputState, List, ListDelegate, Locale, TDesignAssetSource, TDesignConfig,
    TDesignConfigState, TDesignRoot, Table, TableDelegate, ThemeMode, ThemeOverrides, Tree,
    TreeNode, TreeState, Upload, UploadBackend, UploadState,
};

struct Records(Vec<[SharedString; 2]>);

impl ListDelegate for Records {
    fn row_count(&self) -> usize {
        self.0.len()
    }

    fn render_row(&self, index: usize, _window: &mut Window, _cx: &mut App) -> AnyElement {
        div()
            .flex()
            .h_8()
            .children(
                self.0[index]
                    .iter()
                    .cloned()
                    .map(|value| div().flex_1().px_3().child(value)),
            )
            .into_any_element()
    }
}

impl TableDelegate for Records {
    fn column_count(&self) -> usize {
        2
    }

    fn column_name(&self, column: usize) -> SharedString {
        ["ID", "Name"][column].into()
    }
}

struct EchoUploadBackend;

impl UploadBackend for EchoUploadBackend {
    fn upload(&self, path: SharedString, _cx: &mut App) -> Task<anyhow::Result<SharedString>> {
        Task::ready(Ok(format!("local://{path}").into()))
    }
}

struct Patterns {
    config: Entity<TDesignConfigState>,
    input: Entity<InputState>,
    upload: Entity<UploadState>,
    tree: Entity<TreeState>,
    records: Arc<Records>,
    value: String,
}

impl Patterns {
    fn new(cx: &mut Context<Self>) -> Self {
        let config = TDesignConfigState::new(
            cx,
            TDesignConfig::new()
                .theme_mode(ThemeMode::System)
                .theme_overrides(ThemeOverrides::new().brand(gpui::rgb(0x7c3aed)))
                .locale(Locale::en_us()),
        );
        let input = InputState::new(cx, "");
        cx.subscribe(&input, |this, _input, event, cx| {
            let InputEvent::Change(change) = event;
            this.value = change.current.to_string();
            cx.notify();
        })
        .detach();

        let mut root = TreeNode::new("root", "Root").child(TreeNode::new("child", "Child"));
        root.expanded = true;

        Self {
            config,
            input,
            upload: UploadState::new(cx),
            tree: TreeState::new(cx, vec![root]),
            records: Arc::new(Records(
                (0..10_000)
                    .map(|index| [index.to_string().into(), format!("Row {index}").into()])
                    .collect(),
            )),
            value: String::new(),
        }
    }
}

impl Render for Patterns {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        TDesignRoot::new().config_state(self.config.clone()).child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(Input::new(self.input.clone()))
                .child(format!("Input value: {}", self.value))
                .child(
                    Upload::new(self.upload.clone())
                        .backend(Arc::new(EchoUploadBackend))
                        .multiple(true),
                )
                .child(List::new("records", self.records.clone()).height(gpui::px(160.)))
                .child(Table::new("records-table", self.records.clone()).height(gpui::px(200.)))
                .child(Tree::new("navigation-tree", self.tree.clone()).height(gpui::px(160.))),
        )
    }
}

fn main() {
    Application::new()
        .with_assets(TDesignAssetSource::new())
        .run(|cx: &mut App| {
            tdesign_gpui::init(cx);
            cx.open_window(WindowOptions::default(), |_window, cx| {
                cx.new(Patterns::new)
            })
            .expect("open patterns window");
            cx.activate(true);
        });
}
