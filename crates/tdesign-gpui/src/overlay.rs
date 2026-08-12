//! Window-local overlay, notification, and focus-restoration infrastructure.

use gpui::{AnyElement, App, Context, Entity, FocusHandle, Window, prelude::*};
use std::sync::Arc;

/// Stable identifier for an entry in a [`TDesignRoot`](crate::TDesignRoot).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OverlayId(u64);
impl OverlayId {
    /// Returns the numeric identifier.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Layer categories managed by the root overlay host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverlayKind {
    Popup,
    Dialog,
    Drawer,
    Message,
    Notification,
    Popconfirm,
    Guide,
}

type Renderer = Arc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone)]
pub(crate) struct OverlayEntry {
    pub id: OverlayId,
    pub kind: OverlayKind,
    pub modal: bool,
    pub priority: i32,
    pub restore_focus: Option<FocusHandle>,
    pub render: Renderer,
}

/// Entity state used by a root to order layers and restore focus on close.
#[derive(Default)]
pub struct OverlayState {
    next_id: u64,
    entries: Vec<OverlayEntry>,
}
impl OverlayState {
    /// Creates a new GPUI entity.
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self::default())
    }

    /// Adds an overlay. The renderer is invoked once per frame while visible.
    pub fn show(
        &mut self,
        kind: OverlayKind,
        modal: bool,
        priority: i32,
        restore_focus: Option<FocusHandle>,
        render: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) -> OverlayId {
        self.next_id += 1;
        let id = OverlayId(self.next_id);
        self.entries.push(OverlayEntry {
            id,
            kind,
            modal,
            priority,
            restore_focus,
            render: Arc::new(render),
        });
        self.entries.sort_by_key(|entry| (entry.priority, entry.id));
        cx.notify();
        id
    }

    /// Closes one overlay and restores the previously focused element.
    pub fn dismiss(&mut self, id: OverlayId, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        let entry = self.entries.remove(index);
        if let Some(focus) = entry.restore_focus {
            focus.focus(window);
        }
        cx.notify();
        true
    }

    /// Dismisses the top-most layer, which is the layer that should receive
    /// an Escape key by default.
    pub fn dismiss_top(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.entries.last().map(|entry| entry.id) else {
            return false;
        };
        self.dismiss(id, window, cx)
    }

    /// Returns the identifier of the top-most visible layer.
    pub fn top_id(&self) -> Option<OverlayId> {
        self.entries.last().map(|entry| entry.id)
    }

    /// Closes all overlays in reverse stacking order.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(focus) = self
            .entries
            .iter()
            .rev()
            .find_map(|entry| entry.restore_focus.clone())
        {
            focus.focus(window);
        }
        self.entries.clear();
        cx.notify();
    }

    /// Returns whether any modal layer is visible.
    pub fn has_modal(&self) -> bool {
        self.entries.iter().any(|entry| entry.modal)
    }
    /// Returns the number of visible layers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns whether no layer is visible.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn entries(&self) -> Vec<OverlayEntry> {
        self.entries.clone()
    }
}
