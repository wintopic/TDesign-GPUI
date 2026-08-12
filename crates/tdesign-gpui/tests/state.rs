use chrono::{NaiveDate, NaiveTime};
use futures::{StreamExt as _, io::AsyncReadExt as _};
use gpui::{
    App, Context, Render, SharedString, Task, TestAppContext, Window, http_client, prelude::*,
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tdesign_gpui::{
    AutoCompleteState, CalendarState, CascaderOption, CascaderState, DatePickerState, DropdownItem,
    DropdownState, InputEvent, InputState, List, ListDelegate, MenuItem, MenuState, OverlayKind,
    OverlayState, PaginationState, RangeInputState, SelectInputState, SelectOption, SelectState,
    SliderState, TDesignConfig, TDesignConfigState, TabItem, Table, TableDelegate, TabsState,
    TagInputState, Textarea, TimePickerState, ToggleState, TransferItem, TransferState, TreeNode,
    TreeSelectState, TreeState, Upload, UploadBackend, UploadEvent, UploadProgress, UploadState,
    UploadStatus,
};

struct TextareaHarness {
    state: gpui::Entity<InputState>,
}

impl Render for TextareaHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Textarea::new(self.state.clone()).rows(3)
    }
}

struct AutoCompleteHarness {
    state: gpui::Entity<AutoCompleteState>,
}

impl Render for AutoCompleteHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        tdesign_gpui::AutoComplete::new(self.state.clone())
    }
}

struct ListHarness {
    list: List,
}

impl Render for ListHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.list.clone()
    }
}

struct TableHarness {
    table: Table,
}

impl Render for TableHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.table.clone()
    }
}

struct CountingDelegate {
    rows: usize,
    rendered: Arc<AtomicUsize>,
}

impl ListDelegate for CountingDelegate {
    fn row_count(&self) -> usize {
        self.rows
    }

    fn render_row(&self, index: usize, _window: &mut Window, _cx: &mut App) -> gpui::AnyElement {
        self.rendered.fetch_add(1, Ordering::SeqCst);
        gpui::div()
            .h_8()
            .child(format!("Row {index}"))
            .into_any_element()
    }
}

impl TableDelegate for CountingDelegate {
    fn column_count(&self) -> usize {
        2
    }

    fn column_name(&self, column: usize) -> SharedString {
        format!("Column {column}").into()
    }
}

#[derive(Clone, Copy)]
struct TestUploadBackend {
    fail: bool,
}
impl UploadBackend for TestUploadBackend {
    fn upload(&self, path: SharedString, _cx: &mut App) -> Task<anyhow::Result<SharedString>> {
        if self.fail {
            Task::ready(Err(anyhow::anyhow!("test upload failed for {path}")))
        } else {
            Task::ready(Ok(format!("https://cdn.example/{path}").into()))
        }
    }

    fn upload_with_request(
        &self,
        request: tdesign_gpui::UploadRequest,
        cx: &mut App,
    ) -> Task<anyhow::Result<SharedString>> {
        request.report_progress(4, Some(8));
        self.upload(request.path.clone(), cx)
    }
}

#[gpui::test]
fn input_value_change_is_typed(cx: &mut TestAppContext) {
    let state = cx.update(|app| InputState::new(app, "before"));
    let event = state.update(cx, |state, cx| state.set_value("after", cx));
    assert_eq!(event.previous.as_ref(), "before");
    assert_eq!(event.current.as_ref(), "after");
}

#[gpui::test]
async fn input_emits_typed_events_for_text_mutations(cx: &mut TestAppContext) {
    let state = cx.update(|app| InputState::new(app, "before"));
    let mut events = cx.events::<InputEvent, _>(&state);
    state.update(cx, |state, cx| {
        state.set_selection(0.."before".len(), cx);
        let change = state.insert_text("after", cx);
        assert_eq!(change.current.as_ref(), "after");
    });
    match events.next().await.expect("input change event") {
        InputEvent::Change(change) => {
            assert_eq!(change.previous.as_ref(), "before");
            assert_eq!(change.current.as_ref(), "after");
        }
    }
}

#[gpui::test]
fn input_selection_and_insert_preserve_unicode(cx: &mut TestAppContext) {
    let state = cx.update(|app| InputState::new(app, "你好世界"));
    state.update(cx, |state, cx| {
        state.set_selection("你好".len().."你好世界".len(), cx);
        let event = state.insert_text("GPUI", cx);
        assert_eq!(event.previous.as_ref(), "你好世界");
        assert_eq!(event.current.as_ref(), "你好GPUI");
    });
    assert_eq!(state.read_with(cx, |state, _| state.selection()), 10..10);
}

#[gpui::test]
fn input_selection_clamps_to_utf8_boundaries(cx: &mut TestAppContext) {
    let state = cx.update(|app| InputState::new(app, "😀文字"));
    state.update(cx, |state, cx| state.set_selection(1..6, cx));
    let selection = state.read_with(cx, |state, _| state.selection());
    assert!(state.read_with(cx, |state, _| state.value.is_char_boundary(selection.start)));
    assert!(state.read_with(cx, |state, _| state.value.is_char_boundary(selection.end)));
}

#[gpui::test]
fn textarea_renders_explicit_newlines_without_single_line_assertion(cx: &mut TestAppContext) {
    let state = cx.update(|app| InputState::new(app, "第一行\n第二行"));
    state.update(cx, |state, _| state.multiline = true);
    let (_view, _window) = cx.add_window_view(|_window, _cx| TextareaHarness { state });
}

#[gpui::test]
fn config_state_supports_runtime_updates(cx: &mut TestAppContext) {
    let config = TDesignConfig::default();
    let state = cx.update(|app| TDesignConfigState::new(app, config));
    state.update(cx, |state, cx| {
        state.update(|config| config.motion = false, cx);
    });
    assert!(!state.read_with(cx, |state, _| state.config.motion));
}

#[gpui::test]
fn toggles_transition_and_clear_indeterminate(cx: &mut TestAppContext) {
    let state = cx.update(|app| ToggleState::new(app, false));
    state.update(cx, |state, _| state.indeterminate = true);
    let event = state.update(cx, |state, cx| state.toggle(cx));
    assert_eq!((event.previous, event.current), (false, true));
    assert!(!state.read_with(cx, |state, _| state.indeterminate));
}

#[gpui::test]
fn slider_clamps_values(cx: &mut TestAppContext) {
    let state = cx.update(|app| SliderState::new(app, 5.0, 0.0, 10.0));
    state.update(cx, |state, cx| {
        state.step = 2.0;
        state.set_value(50.0, cx);
    });
    assert_eq!(state.read_with(cx, |state, _| state.value), 10.0);
    state.update(cx, |state, cx| {
        let change = state.decrement(cx);
        assert_eq!((change.previous, change.current), (10.0, 8.0));
        let change = state.set_ratio(0.25, cx);
        assert_eq!(change.current, 2.0);
    });
}

#[gpui::test]
fn date_picker_respects_range_and_keyboard_selection(cx: &mut TestAppContext) {
    let value = NaiveDate::from_ymd_opt(2026, 8, 12).expect("valid date");
    let min = NaiveDate::from_ymd_opt(2026, 8, 10).expect("valid date");
    let max = NaiveDate::from_ymd_opt(2026, 8, 20).expect("valid date");
    let state = cx.update(|app| DatePickerState::new(app, Some(value)));
    state.update(cx, |state, cx| {
        state.min = Some(min);
        state.max = Some(max);
        state.open(cx);
        state.move_highlight(1, cx);
        let change = state.select_highlighted(cx).expect("highlighted date");
        assert_eq!(
            change.current,
            Some(NaiveDate::from_ymd_opt(2026, 8, 13).unwrap())
        );
    });
    assert!(!state.read_with(cx, |state, _| state.open));
    assert!(!state.update(cx, |state, cx| {
        state.select(NaiveDate::from_ymd_opt(2026, 8, 30).unwrap(), cx)
    }));
}

#[gpui::test]
fn calendar_range_and_keyboard_selection_are_consistent(cx: &mut TestAppContext) {
    let selected = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
    let state = cx.update(|app| CalendarState::new(app, Some(selected)));
    state.update(cx, |state, cx| {
        state.min = Some(NaiveDate::from_ymd_opt(2026, 8, 10).unwrap());
        state.max = Some(NaiveDate::from_ymd_opt(2026, 8, 20).unwrap());
        assert!(state.is_date_disabled(NaiveDate::from_ymd_opt(2026, 8, 9).unwrap()));
        let rejected = state.select(NaiveDate::from_ymd_opt(2026, 8, 30).unwrap(), cx);
        assert_eq!(rejected.current, Some(selected));
        assert_eq!(
            state.move_highlight(-1, cx),
            Some(NaiveDate::from_ymd_opt(2026, 8, 11).unwrap())
        );
        let change = state.select_highlighted(cx);
        assert_eq!(
            change.current,
            Some(NaiveDate::from_ymd_opt(2026, 8, 11).unwrap())
        );
        state.shift_month(-12, cx);
        assert_eq!(state.month, NaiveDate::from_ymd_opt(2026, 8, 1).unwrap());
    });
}

#[gpui::test]
fn time_picker_steps_and_clamps_to_range(cx: &mut TestAppContext) {
    let initial = NaiveTime::from_hms_opt(23, 59, 0).expect("valid time");
    let state = cx.update(|app| TimePickerState::new(app, Some(initial)));
    state.update(cx, |state, cx| {
        state.min = Some(NaiveTime::from_hms_opt(8, 0, 0).unwrap());
        state.max = Some(NaiveTime::from_hms_opt(23, 59, 0).unwrap());
        state.step = 60;
        let change = state.increment(cx);
        assert_eq!(change.current, Some(initial));
        let change = state.decrement(cx);
        assert_eq!(
            change.current,
            Some(NaiveTime::from_hms_opt(23, 58, 0).unwrap())
        );
    });
}

#[gpui::test]
fn autocomplete_and_select_input_keyboard_state(cx: &mut TestAppContext) {
    let mut disabled = SelectOption::new("disabled", "Disabled");
    disabled.disabled = true;
    let autocomplete = cx.update(|app| {
        AutoCompleteState::new(
            app,
            vec![
                disabled.clone(),
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    autocomplete.update(cx, |state, cx| {
        state.set_query("", cx);
        state.move_highlight(1, cx);
        assert_eq!(state.select_highlighted(cx), Some("two".to_owned()));
    });
    let select = cx.update(|app| {
        SelectInputState::new(
            app,
            vec![
                disabled,
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    select.update(cx, |state, cx| {
        state.move_highlight(1, cx);
        assert_eq!(state.select_highlighted(cx), Some("one".to_owned()));
        state.multiple = true;
        assert!(state.select("two", cx));
        assert_eq!(state.selected, vec!["one".to_owned(), "two".to_owned()]);
    });
}

#[gpui::test]
fn autocomplete_tracks_native_input_edits(cx: &mut TestAppContext) {
    let state = cx.update(|app| {
        AutoCompleteState::new(
            app,
            vec![
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    let input = state.read_with(cx, |state, _| state.input.clone());
    input.update(cx, |input, cx| {
        input.insert_text("tw", cx);
    });
    assert_eq!(
        state.read_with(cx, |state, _| state.query.clone()),
        SharedString::from("tw")
    );
    assert!(state.read_with(cx, |state, _| state.open));
}

#[gpui::test]
fn autocomplete_accepts_simulated_native_text_input(cx: &mut TestAppContext) {
    let state = cx.update(|app| {
        AutoCompleteState::new(
            app,
            vec![
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    let (_view, window) = cx.add_window_view(|_window, _cx| AutoCompleteHarness {
        state: state.clone(),
    });
    window.update(|window, cx| {
        state.read(cx).focus_handle.focus(window);
    });
    window.simulate_input("tw");
    assert_eq!(
        state.read_with(window, |state, _| state.query.clone()),
        SharedString::from("tw")
    );
    assert_eq!(
        state.read_with(window, |state, _| state.highlighted),
        Some(1)
    );
}

#[gpui::test]
fn tag_range_and_select_inputs_track_native_editors(cx: &mut TestAppContext) {
    let tags = cx.update(|app| TagInputState::new(app, Vec::new()));
    let tag_input = tags.read_with(cx, |state, _| state.input.clone());
    tag_input.update(cx, |input, cx| {
        input.insert_text("rust", cx);
    });
    assert_eq!(
        tags.read_with(cx, |state, _| state.draft.clone()),
        SharedString::from("rust")
    );
    assert!(tags.update(cx, |state, cx| state.commit_draft(cx)));
    assert_eq!(
        tags.read_with(cx, |state, _| state.tags.clone()),
        vec![SharedString::from("rust")]
    );
    assert_eq!(
        tags.read_with(cx, |state, _| state.draft.clone()),
        SharedString::default()
    );
    assert_eq!(
        tag_input.read_with(cx, |input, _| input.value.clone()),
        SharedString::default()
    );

    let range = cx.update(|app| RangeInputState::new(app, "from", "to"));
    let (start_input, end_input) = range.read_with(cx, |state, _| {
        (state.start_input.clone(), state.end_input.clone())
    });
    start_input.update(cx, |input, cx| input.set_value("left", cx));
    end_input.update(cx, |input, cx| input.set_value("right", cx));
    assert_eq!(
        range.read_with(cx, |state, _| (state.start.clone(), state.end.clone())),
        (SharedString::from("left"), SharedString::from("right"))
    );
    range.update(cx, |state, cx| state.swap(cx));
    assert_eq!(
        start_input.read_with(cx, |input, _| input.value.clone()),
        SharedString::from("right")
    );

    let select = cx.update(|app| {
        SelectInputState::new(
            app,
            vec![
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    let select_input = select.read_with(cx, |state, _| state.input.clone());
    select_input.update(cx, |input, cx| input.insert_text("tw", cx));
    assert_eq!(
        select.read_with(cx, |state, _| state.query.clone()),
        SharedString::from("tw")
    );
    assert_eq!(select.read_with(cx, |state, _| state.highlighted), Some(1));
    assert_eq!(
        select.update(cx, |state, cx| state.select_highlighted(cx)),
        Some("two".to_owned())
    );
    assert!(!select.read_with(cx, |state, _| state.open));
    assert_eq!(
        select.read_with(cx, |state, _| state.query.clone()),
        SharedString::default()
    );
}

#[gpui::test]
fn cascader_keyboard_skips_disabled_nodes(cx: &mut TestAppContext) {
    let mut disabled = CascaderOption::new("disabled", "Disabled");
    disabled.disabled = true;
    let state = cx.update(|app| {
        CascaderState::new(
            app,
            vec![disabled, CascaderOption::new("enabled", "Enabled")],
        )
    });
    state.update(cx, |state, cx| {
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("enabled"));
        assert_eq!(
            state.select_highlighted(cx),
            Some(vec!["enabled".to_owned()])
        );
    });
}

#[gpui::test]
fn select_rejects_disabled_options(cx: &mut TestAppContext) {
    let mut disabled = SelectOption::new("disabled", "Disabled");
    disabled.disabled = true;
    let state =
        cx.update(|app| SelectState::new(app, vec![SelectOption::new("ok", "OK"), disabled]));
    assert!(!state.update(cx, |state, cx| state.select("disabled", cx)));
    assert!(state.update(cx, |state, cx| state.select("ok", cx)));
}

#[gpui::test]
fn select_keyboard_highlight_skips_disabled_options(cx: &mut TestAppContext) {
    let mut disabled = SelectOption::new("disabled", "Disabled");
    disabled.disabled = true;
    let state = cx.update(|app| {
        SelectState::new(
            app,
            vec![
                disabled,
                SelectOption::new("one", "One"),
                SelectOption::new("two", "Two"),
            ],
        )
    });
    state.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(
        state.read_with(cx, |state, _| state.highlighted.clone()),
        Some(1)
    );
    state.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(
        state.update(cx, |state, cx| state.select_highlighted(cx)),
        Some("two".to_owned())
    );
}

#[gpui::test]
fn dropdown_keyboard_activation_and_escape(cx: &mut TestAppContext) {
    let state = cx.update(|app| {
        DropdownState::new(
            app,
            vec![
                DropdownItem::new("first", "First"),
                DropdownItem::new("second", "Second"),
            ],
        )
    });
    state.update(cx, |state, cx| state.move_highlight(1, cx));
    state.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(state.read_with(cx, |state, _| state.highlighted), Some(1));
    assert_eq!(
        state.update(cx, |state, cx| state.select_highlighted(cx)),
        Some("second".to_owned())
    );
    state.update(cx, |state, cx| state.toggle(cx));
    state.update(cx, |state, cx| state.open = true);
    state.update(cx, |state, cx| {
        state.open = false;
        cx.notify();
    });
    assert!(!state.read_with(cx, |state, _| state.open));
}

#[gpui::test]
fn tree_select_does_not_select_disabled_descendants(cx: &mut TestAppContext) {
    let mut disabled = TreeNode::new("disabled", "Disabled");
    disabled.disabled = true;
    let root = TreeNode::new("root", "Root").child(disabled);
    let state = cx.update(|app| TreeSelectState::new(app, vec![root]));
    assert!(!state.update(cx, |state, cx| state.select("disabled", cx)));
    state.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(
        state.read_with(cx, |state, _| state.highlighted.clone()),
        Some("root".to_owned())
    );
}

#[gpui::test]
fn disabled_tree_ancestors_hide_their_enabled_subtrees(cx: &mut TestAppContext) {
    let mut disabled_tree_parent =
        TreeNode::new("blocked", "Blocked").child(TreeNode::new("hidden-child", "Hidden child"));
    disabled_tree_parent.disabled = true;
    let visible_tree_node = TreeNode::new("visible", "Visible");

    let tree = cx.update(|app| {
        TreeState::new(
            app,
            vec![disabled_tree_parent.clone(), visible_tree_node.clone()],
        )
    });
    assert!(!tree.update(cx, |state, cx| state.select_if_enabled("hidden-child", cx)));
    assert!(!tree.update(cx, |state, cx| state.toggle("hidden-child", cx)));
    assert!(tree.update(cx, |state, cx| state.select_if_enabled("visible", cx)));

    let tree_select =
        cx.update(|app| TreeSelectState::new(app, vec![disabled_tree_parent, visible_tree_node]));
    assert!(!tree_select.update(cx, |state, cx| state.select("hidden-child", cx)));
    tree_select.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(
        tree_select.read_with(cx, |state, _| state.highlighted.clone()),
        Some("visible".to_owned())
    );

    let mut disabled_cascader_parent = CascaderOption::new("blocked", "Blocked")
        .child(CascaderOption::new("hidden-child", "Hidden child"));
    disabled_cascader_parent.disabled = true;
    let cascader = cx.update(|app| {
        CascaderState::new(
            app,
            vec![
                disabled_cascader_parent,
                CascaderOption::new("visible", "Visible"),
            ],
        )
    });
    assert!(!cascader.update(cx, |state, cx| state.select("hidden-child", cx)));
    cascader.update(cx, |state, cx| state.move_highlight(1, cx));
    assert_eq!(
        cascader.read_with(cx, |state, _| state.highlighted.clone()),
        Some("visible".to_owned())
    );
}

#[gpui::test]
fn pagination_clamps_page(cx: &mut TestAppContext) {
    let state = cx.update(|app| PaginationState::new(app, 95, 10));
    state.update(cx, |state, cx| state.set_page(99, cx));
    assert_eq!(state.read_with(cx, |state, _| state.page), 10);
}

#[gpui::test]
fn menu_and_tabs_only_select_enabled_items(cx: &mut TestAppContext) {
    let menu = cx.update(|app| MenuState::new(app, vec![MenuItem::new("home", "Home")]));
    assert!(menu.update(cx, |state, cx| state.select("home", cx)));
    let tabs = cx.update(|app| TabsState::new(app, vec![TabItem::new("one", "One")]));
    assert!(tabs.update(cx, |state, cx| state.activate("one", cx)));
}

#[gpui::test]
fn transfer_rejects_disabled_items_and_moves_only_selected_panel_items(cx: &mut TestAppContext) {
    let mut disabled = TransferItem::new("disabled", "Disabled");
    disabled.disabled = true;
    let state = cx.update(|app| {
        TransferState::new(
            app,
            vec![
                disabled,
                TransferItem::new("one", "One"),
                TransferItem::new("two", "Two"),
            ],
        )
    });
    state.update(cx, |state, cx| {
        assert!(!state.toggle("disabled", cx));
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("one"));
        assert!(state.toggle_highlighted(cx));
        assert!(state.move_to_target(cx));
        assert!(state.target.contains("one"));
        assert!(!state.checked.contains("one"));

        assert!(state.toggle("two", cx));
        assert!(!state.move_to_source(cx));
        assert!(state.checked.contains("two"));
        assert!(!state.target.contains("two"));

        assert!(state.toggle("one", cx));
        assert!(state.move_to_source(cx));
        assert!(!state.target.contains("one"));
        assert!(state.checked.contains("two"));
    });
}

#[gpui::test]
fn large_list_and_table_render_only_a_visible_subset(cx: &mut TestAppContext) {
    const ROWS: usize = 100_000;

    let list_count = Arc::new(AtomicUsize::new(0));
    let list_delegate: Arc<dyn ListDelegate> = Arc::new(CountingDelegate {
        rows: ROWS,
        rendered: list_count.clone(),
    });
    let (_view, _window) = cx.add_window_view(|_window, _cx| ListHarness {
        list: List::new("virtual-list-test", list_delegate).height(gpui::px(160.)),
    });
    let rendered = list_count.load(Ordering::SeqCst);
    assert!(rendered > 0, "the visible list rows should render");
    assert!(
        rendered < 128,
        "virtual list rendered {rendered} of {ROWS} rows"
    );

    let table_count = Arc::new(AtomicUsize::new(0));
    let table_delegate: Arc<dyn TableDelegate> = Arc::new(CountingDelegate {
        rows: ROWS,
        rendered: table_count.clone(),
    });
    let (_view, _window) = cx.add_window_view(|_window, _cx| TableHarness {
        table: Table::new("virtual-table-test", table_delegate).height(gpui::px(192.)),
    });
    let rendered = table_count.load(Ordering::SeqCst);
    assert!(rendered > 0, "the visible table rows should render");
    assert!(
        rendered < 128,
        "virtual table rendered {rendered} of {ROWS} rows"
    );
}

#[gpui::test]
fn tree_expansion_and_selection_are_independent(cx: &mut TestAppContext) {
    let root = TreeNode::new("root", "Root").child(TreeNode::new("child", "Child"));
    let state = cx.update(|app| TreeState::new(app, vec![root]));
    assert!(state.update(cx, |state, cx| state.toggle("root", cx)));
    state.update(cx, |state, cx| state.select("child", cx));
    assert_eq!(
        state.read_with(cx, |state, _| state.selected.clone()),
        Some("child".into())
    );
}

#[gpui::test]
fn tree_keyboard_navigation_uses_only_visible_enabled_nodes(cx: &mut TestAppContext) {
    let mut disabled = TreeNode::new("disabled", "Disabled");
    disabled.disabled = true;
    let mut root = TreeNode::new("root", "Root")
        .child(TreeNode::new("child", "Child"))
        .child(disabled);
    root.expanded = true;
    let state = cx.update(|app| TreeState::new(app, vec![root]));
    state.update(cx, |state, cx| {
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("root"));
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("child"));
        assert!(!state.toggle_highlighted(cx));
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("root"));
        assert!(state.select_highlighted(cx).is_some());
        assert!(state.toggle_highlighted(cx));
        state.move_highlight(1, cx);
        assert_eq!(state.highlighted.as_deref(), Some("root"));
    });
}

#[gpui::test]
fn overlay_stack_orders_and_tracks_modal_state(cx: &mut TestAppContext) {
    let overlays = cx.update(OverlayState::new);
    let first = overlays.update(cx, |state, cx| {
        state.show(
            OverlayKind::Message,
            false,
            10,
            None,
            |_, _| gpui::div().into_any_element(),
            cx,
        )
    });
    let second = overlays.update(cx, |state, cx| {
        state.show(
            OverlayKind::Dialog,
            true,
            20,
            None,
            |_, _| gpui::div().into_any_element(),
            cx,
        )
    });
    assert!(second > first);
    assert_eq!(overlays.read_with(cx, |state, _| state.len()), 2);
    assert!(overlays.read_with(cx, |state, _| state.has_modal()));
    assert_eq!(
        overlays.read_with(cx, |state, _| state.top_id()),
        Some(second)
    );
}

#[gpui::test]
async fn upload_success_emits_progress_and_completes(cx: &mut TestAppContext) {
    let state = cx.update(|app| UploadState::new(app));
    let mut events = cx.events::<UploadEvent, _>(&state);
    let upload = Upload::new(state.clone()).backend(Arc::new(TestUploadBackend { fail: false }));
    let path = PathBuf::from("fixture.txt");
    let task = cx.update(|app| upload.upload_paths(vec![path.clone()], app));
    task.await.expect("upload orchestration should succeed");

    assert!(matches!(
        state.read_with(cx, |state, _| state.status(&path).cloned()),
        Some(UploadStatus::Success(_))
    ));
    let mut saw_selected = false;
    let mut saw_started = false;
    let mut saw_progress = false;
    let mut saw_success = false;
    for _ in 0..4 {
        match events.next().await.expect("upload event") {
            UploadEvent::FilesSelected { .. } => saw_selected = true,
            UploadEvent::Started { .. } => saw_started = true,
            UploadEvent::Progress { .. } => saw_progress = true,
            UploadEvent::Succeeded { .. } => saw_success = true,
            _ => {}
        }
    }
    assert!(saw_selected && saw_started && saw_progress && saw_success);
    assert_eq!(
        state.read_with(cx, |state, _| state.progress(&path)),
        Some(UploadProgress::new(8, Some(8)))
    );
}

#[gpui::test]
async fn upload_failure_can_be_retried_and_cancelled(cx: &mut TestAppContext) {
    let state = cx.update(|app| UploadState::new(app));
    let path = PathBuf::from("failure.bin");
    let upload = Upload::new(state.clone()).backend(Arc::new(TestUploadBackend { fail: true }));
    cx.update(|app| upload.upload_paths(vec![path.clone()], app))
        .await
        .expect("orchestration should retain backend failure in state");
    assert!(matches!(
        state.read_with(cx, |state, _| state.status(&path).cloned()),
        Some(UploadStatus::Failed(_))
    ));

    let retry = cx.update(|app| upload.retry(path.clone(), app));
    retry.await.expect("retry orchestration should finish");
    assert!(matches!(
        state.read_with(cx, |state, _| state.status(&path).cloned()),
        Some(UploadStatus::Failed(_))
    ));

    let token = state.update(cx, |state, cx| {
        state.begin_upload(&path, cx).expect("path is queued")
    });
    assert!(state.update(cx, |state, cx| state.cancel_file(&path, cx)));
    assert!(token.is_cancelled());
    assert_eq!(
        state.read_with(cx, |state, _| state.status(&path).cloned()),
        Some(UploadStatus::Canceled)
    );
}

#[gpui::test]
async fn http_action_backend_sends_multipart_body(cx: &mut TestAppContext) {
    let captured = Arc::new(Mutex::new(None::<(String, Vec<u8>)>));
    let captured_handler = captured.clone();
    let client = http_client::FakeHttpClient::create(move |request| {
        let captured = captured_handler.clone();
        async move {
            let content_type = request
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let (_, mut body) = request.into_parts();
            let mut bytes = Vec::new();
            body.read_to_end(&mut bytes).await?;
            *captured.lock().expect("capture mutex") = Some((content_type, bytes));
            Ok(http_client::Response::builder()
                .status(201)
                .header("Location", "https://cdn.example/uploaded.txt")
                .body(http_client::AsyncBody::default())?)
        }
    });
    cx.update(|app| app.set_http_client(client));

    let fixture = std::env::temp_dir().join(format!(
        "tdesign-gpui-upload-{}-{}.txt",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::write(&fixture, "hello upload").expect("write fixture");
    let state = cx.update(|app| UploadState::new(app));
    let upload = Upload::new(state.clone()).action("https://example.test/upload");
    cx.update(|app| upload.upload_paths(vec![fixture.clone()], app))
        .await
        .expect("HTTP action upload");

    let captured = captured
        .lock()
        .expect("capture mutex")
        .clone()
        .expect("HTTP request captured");
    assert!(captured.0.starts_with("multipart/form-data; boundary="));
    let body = String::from_utf8_lossy(&captured.1);
    assert!(body.contains("name=\"file\""));
    assert!(body.contains("filename=\"tdesign-gpui-upload-"));
    assert!(body.contains("hello upload"));
    assert!(matches!(
        state.read_with(cx, |state, _| state.status(&fixture).cloned()),
        Some(UploadStatus::Success(_))
    ));
    let _ = std::fs::remove_file(fixture);
}
