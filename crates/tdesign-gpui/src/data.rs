//! Virtualized collection components.

use crate::{ListDelegate, TableDelegate, TreeNode};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, IntoElement, ParentElement, RenderOnce,
    SharedString, Window, div, prelude::*, px, uniform_list,
};
use std::sync::Arc;

/// A virtualized uniform-height list.
#[derive(Clone, IntoElement)]
pub struct List {
    id: SharedString,
    delegate: Arc<dyn ListDelegate>,
    height: gpui::Pixels,
}
impl List {
    /// Creates a list using an application delegate.
    pub fn new(id: impl Into<SharedString>, delegate: Arc<dyn ListDelegate>) -> Self {
        Self {
            id: id.into(),
            delegate,
            height: px(320.),
        }
    }
    /// Sets viewport height.
    pub fn height(mut self, height: impl Into<gpui::Pixels>) -> Self {
        self.height = height.into();
        self
    }
}
impl RenderOnce for List {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let count = self.delegate.row_count();
        let delegate = self.delegate;
        uniform_list(self.id, count, move |range, window, cx| {
            let row_count = delegate.row_count();
            range
                .take_while(|index| *index < row_count)
                .map(|index| delegate.render_row(index, window, cx))
                .collect::<Vec<AnyElement>>()
        })
        .h(self.height)
    }
}
/// List module.
pub mod list {
    pub use super::List;
    pub use crate::ListDelegate;
}

/// A virtualized table whose delegate renders visible rows.
#[derive(Clone, IntoElement)]
pub struct Table {
    id: SharedString,
    delegate: Arc<dyn TableDelegate>,
    height: gpui::Pixels,
    striped: bool,
}
impl Table {
    /// Creates a table.
    pub fn new(id: impl Into<SharedString>, delegate: Arc<dyn TableDelegate>) -> Self {
        Self {
            id: id.into(),
            delegate,
            height: px(360.),
            striped: true,
        }
    }
    /// Sets viewport height.
    pub fn height(mut self, height: impl Into<gpui::Pixels>) -> Self {
        self.height = height.into();
        self
    }
    /// Enables alternating row backgrounds around delegate-rendered rows.
    pub fn striped(mut self, striped: bool) -> Self {
        self.striped = striped;
        self
    }
}
impl RenderOnce for Table {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let header = (0..self.delegate.column_count())
            .map(|column| {
                div()
                    .flex_1()
                    .px_3()
                    .py_2()
                    .child(self.delegate.column_name(column))
            })
            .collect::<Vec<_>>();
        let count = self.delegate.row_count();
        let delegate = self.delegate;
        let striped = self.striped;
        let list_id: SharedString = format!("{}-rows", self.id).into();
        div()
            .id(self.id)
            .flex()
            .flex_col()
            .h(self.height)
            .rounded_sm()
            .border_1()
            .border_color(gpui::rgb(0xe7e7e7))
            .child(
                div()
                    .flex()
                    .bg(gpui::rgb(0xf3f3f3))
                    .text_color(gpui::rgb(0x333333))
                    .children(header),
            )
            .child(
                uniform_list(list_id, count, move |range, window, cx| {
                    let row_count = delegate.row_count();
                    range
                        .take_while(|index| *index < row_count)
                        .map(|index| {
                            div()
                                .when(striped && index % 2 == 1, |row| row.bg(gpui::rgb(0xf7f7f7)))
                                .child(delegate.render_row(index, window, cx))
                                .into_any_element()
                        })
                        .collect::<Vec<AnyElement>>()
                })
                .flex_1(),
            )
    }
}
/// Table module.
pub mod table {
    pub use super::Table;
    pub use crate::TableDelegate;
}

/// Entity state for expansion and selection in a tree.
#[derive(Debug)]
pub struct TreeState {
    pub roots: Vec<TreeNode>,
    pub selected: Option<String>,
    /// Key highlighted for keyboard traversal.
    pub highlighted: Option<String>,
    pub disabled: bool,
    pub focus_handle: FocusHandle,
}
impl TreeState {
    /// Creates tree state.
    pub fn new(cx: &mut App, roots: Vec<TreeNode>) -> Entity<Self> {
        cx.new(|cx| Self {
            roots,
            selected: None,
            highlighted: None,
            disabled: false,
            focus_handle: cx.focus_handle(),
        })
    }
    /// Selects a node by stable key.
    pub fn select(&mut self, key: impl Into<String>, cx: &mut Context<Self>) {
        let key = key.into();
        let _ = self.select_if_enabled(&key, cx);
    }

    /// Selects an existing, enabled node and reports whether the selection changed.
    pub fn select_if_enabled(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if self.disabled || !tree_contains_enabled(&self.roots, key) {
            return false;
        }
        if self.selected.as_deref() == Some(key) {
            return true;
        }
        self.selected = Some(key.to_owned());
        self.highlighted = Some(key.to_owned());
        cx.notify();
        true
    }
    /// Toggles a node's expansion state.
    pub fn toggle(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        fn visit(nodes: &mut [TreeNode], key: &str) -> bool {
            for node in nodes {
                if node.disabled {
                    continue;
                }
                if node.key == key {
                    if node.children.is_empty() {
                        return false;
                    }
                    node.expanded = !node.expanded;
                    return true;
                }
                if visit(&mut node.children, key) {
                    return true;
                }
            }
            false
        }
        let found = visit(&mut self.roots, key);
        if found {
            cx.notify();
        }
        found
    }

    /// Moves keyboard highlight through visible, enabled nodes.
    pub fn move_highlight(&mut self, delta: i32, cx: &mut Context<Self>) {
        let mut nodes = Vec::new();
        collect_visible_enabled_keys(&self.roots, &mut nodes);
        if self.disabled || nodes.is_empty() {
            return;
        }
        let current = self
            .highlighted
            .as_ref()
            .or(self.selected.as_ref())
            .and_then(|key| nodes.iter().position(|candidate| candidate == key));
        let next = match current {
            Some(current) => (current as i32 + delta).rem_euclid(nodes.len() as i32) as usize,
            None if delta < 0 => nodes.len() - 1,
            None => 0,
        };
        self.highlighted = Some(nodes[next].clone());
        cx.notify();
    }

    /// Selects the highlighted enabled node.
    pub fn select_highlighted(&mut self, cx: &mut Context<Self>) -> Option<String> {
        let key = self.highlighted.clone()?;
        self.select_if_enabled(&key, cx).then_some(key)
    }

    /// Expands/collapses the highlighted node when it has children.
    pub fn toggle_highlighted(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(key) = self.highlighted.clone() else {
            return false;
        };
        self.toggle(&key, cx)
    }
}

fn tree_contains_enabled(nodes: &[TreeNode], key: &str) -> bool {
    nodes.iter().any(|node| {
        if node.disabled {
            return false;
        }
        if node.key == key {
            true
        } else {
            tree_contains_enabled(&node.children, key)
        }
    })
}

fn collect_visible_enabled_keys(nodes: &[TreeNode], keys: &mut Vec<String>) {
    for node in nodes {
        if node.disabled {
            continue;
        }
        keys.push(node.key.clone());
        if node.expanded {
            collect_visible_enabled_keys(&node.children, keys);
        }
    }
}

#[derive(Clone)]
struct FlatNode {
    key: String,
    label: SharedString,
    depth: usize,
    expanded: bool,
    has_children: bool,
    disabled: bool,
}
fn flatten(nodes: &[TreeNode], depth: usize, output: &mut Vec<FlatNode>) {
    for node in nodes {
        output.push(FlatNode {
            key: node.key.clone(),
            label: node.label.clone(),
            depth,
            expanded: node.expanded,
            has_children: !node.children.is_empty(),
            disabled: node.disabled,
        });
        if node.expanded {
            flatten(&node.children, depth + 1, output);
        }
    }
}

/// A virtualized expandable tree.
#[derive(Clone, Debug, IntoElement)]
pub struct Tree {
    state: Entity<TreeState>,
    id: SharedString,
    height: gpui::Pixels,
}
impl Tree {
    /// Creates a tree.
    pub fn new(id: impl Into<SharedString>, state: Entity<TreeState>) -> Self {
        Self {
            state,
            id: id.into(),
            height: px(360.),
        }
    }
    /// Sets viewport height.
    pub fn height(mut self, value: impl Into<gpui::Pixels>) -> Self {
        self.height = value.into();
        self
    }
}
impl RenderOnce for Tree {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let mut flat = Vec::new();
        flatten(&state.roots, 0, &mut flat);
        let selected = state.selected.clone();
        let highlighted = state.highlighted.clone();
        let focus = state.focus_handle.clone();
        let entity = self.state.clone();
        let key_entity = self.state.clone();
        uniform_list(self.id, flat.len(), move |range, _window, _cx| {
            range
                .map(|index| {
                    let node = flat[index].clone();
                    let key = node.key.clone();
                    let entity = entity.clone();
                    let is_selected = selected.as_ref() == Some(&node.key);
                    let is_highlighted = highlighted.as_ref() == Some(&node.key);
                    div()
                        .id(SharedString::from(format!("tree-node-{}", node.key)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .h_7()
                        .pl(px(8. + node.depth as f32 * 20.))
                        .pr_2()
                        .when(is_selected, |this| {
                            this.bg(gpui::rgb(0xe7f1ff)).text_color(gpui::rgb(0x0052d9))
                        })
                        .when(is_highlighted && !is_selected, |this| {
                            this.border_1().border_color(gpui::rgb(0x0052d9))
                        })
                        .when(node.disabled, |this| this.opacity(0.5))
                        .child(if node.has_children {
                            if node.expanded { "⌄" } else { "›" }
                        } else {
                            " "
                        })
                        .child(node.label)
                        .on_click(move |_, _, cx| {
                            if node.disabled {
                                return;
                            }
                            let _ = entity.update(cx, |state, cx| {
                                if node.has_children {
                                    state.toggle(&key, cx);
                                }
                                state.select(key.clone(), cx);
                            });
                        })
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        .track_focus(&focus)
        .h(self.height)
        .on_key_down(move |event, _, cx| {
            if key_entity.read(cx).disabled {
                return;
            }
            let key = event.keystroke.key.to_ascii_lowercase();
            match key.as_str() {
                "down" | "arrowdown" => {
                    cx.stop_propagation();
                    key_entity.update(cx, |state, cx| state.move_highlight(1, cx));
                }
                "up" | "arrowup" => {
                    cx.stop_propagation();
                    key_entity.update(cx, |state, cx| state.move_highlight(-1, cx));
                }
                "enter" | "space" => {
                    cx.stop_propagation();
                    key_entity.update(cx, |state, cx| state.select_highlighted(cx));
                }
                "left" | "arrowleft" | "right" | "arrowright" => {
                    cx.stop_propagation();
                    key_entity.update(cx, |state, cx| state.toggle_highlighted(cx));
                }
                _ => {}
            }
        })
    }
}
/// Tree module.
pub mod tree {
    pub use super::{Tree, TreeState};
    pub use crate::TreeNode;
}
