use std::path::PathBuf;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use tracing::{debug, error, info};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::ListState,
};
use tui_textarea::{Input, TextArea};

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::config::{AppConfig, Connection, Profiles};
use crate::db::{self, Database, QueryResult, SchemaNode};
use crate::keybindings::{Keybindings, LeaderEntry};
use crate::recents::{RecentEntry, Recents};
use crate::tree::TreeNode;
use crate::vim::{self, Transition, Vim};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Focus {
    Sidebar,
    Files,
    QueryEditor,
    Results,
    Recent,
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum QueryLanguage {
    #[default]
    Sql,
    Prql,
}

impl QueryLanguage {
    pub fn display(&self) -> &'static str {
        match self {
            QueryLanguage::Sql => "SQL",
            QueryLanguage::Prql => "PRQL",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            QueryLanguage::Sql => QueryLanguage::Prql,
            QueryLanguage::Prql => QueryLanguage::Sql,
        }
    }

    /// Whether the SQL preview pane should be visible by default for this language.
    pub fn preview_by_default(&self) -> bool {
        !matches!(self, QueryLanguage::Sql)
    }

    /// Transpile the given source to SQL. Returns `Err` with a human-readable
    /// message if the source cannot be compiled (e.g. invalid PRQL).
    pub fn transpile(&self, source: &str) -> Result<String, String> {
        match self {
            QueryLanguage::Sql => Ok(source.to_string()),
            QueryLanguage::Prql => {
                use prqlc::{Options, Target, compile};
                let opts = Options::default()
                    .with_target(Target::Sql(None))
                    .with_signature_comment(false);
                compile(source, &opts).map_err(|e| e.to_string())
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MessageLevel {
    Info,
    Error,
}

pub struct Message {
    pub text: String,
    pub level: MessageLevel,
}

#[derive(Debug, PartialEq)]
pub enum SidebarNodeKind {
    Connection,
    TableOrView,
    Other,
}

pub const RESULTS_PAGE_SIZE: usize = 100;

pub enum BgResult {
    Connected {
        label: String,
        result: Result<(Box<dyn Database>, Vec<SchemaNode>), String>,
    },
    Query {
        conn: Box<dyn Database>,
        result: Result<(QueryResult, Duration, bool), String>,
    },
}

pub struct App<'a> {
    pub sidebar_items: Vec<TreeNode>,
    pub sidebar_state: ListState,
    pub editor: TextArea<'a>,
    pub vim: Vim,
    pub results_visible: bool,
    pub query_result: Option<db::QueryResult>,
    pub focus: Focus,
    pub running: bool,
    pub sidebar_width: u16,
    pub connected_db: Option<String>,
    pub message: Option<Message>,
    pub profiles: Profiles,
    pub connection: Option<Box<dyn Database>>,
    pub query_duration: Option<Duration>,
    pub show_help: bool,
    pub keys: Keybindings,
    label_to_profile: BTreeMap<String, String>,
    // Results navigation
    pub results_scroll_row: usize,
    pub results_scroll_col: usize,
    pub results_page: usize,
    pub results_has_more: bool,
    results_query: Option<String>,
    pub results_area: Rect,
    pub loading: Option<String>,
    pub spinner_tick: usize,
    bg_receiver: Option<mpsc::Receiver<BgResult>>,
    pub sidebar_filter: String,
    pub sidebar_filtering: bool,
    pub file_filter: String,
    pub file_filtering: bool,
    pub leader_active: bool,
    pub show_sidebar: bool,
    pub show_files: bool,
    pub show_recent: bool,
    pub recents: Recents,
    pub recents_selected: usize,
    max_recents: usize,
    // Files pane
    pub file_tree: Vec<TreeNode>,
    pub file_tree_state: ListState,
    pub files_root: Option<PathBuf>,
    pub file_paths: Vec<PathBuf>,
    pub query_language: QueryLanguage,
    pub show_sql_preview: bool,
}

impl<'a> App<'a> {
    pub fn new(config: AppConfig, profiles: Profiles, files_root: Option<PathBuf>) -> Self {
        let (sidebar_items, label_to_profile) = build_sidebar_tree(&profiles);

        let mut state = ListState::default();
        if !sidebar_items.is_empty() {
            state.select(Some(0));
        }

        let (file_tree, file_paths, has_files) = if let Some(ref root) = files_root {
            let tree = crate::files::build_file_tree(root);
            let paths = crate::files::build_path_index(&tree, root);
            let has = !tree.is_empty();
            (tree, paths, has)
        } else {
            (vec![], vec![], false)
        };
        let mut file_tree_state = ListState::default();
        if !file_tree.is_empty() {
            file_tree_state.select(Some(0));
        }

        let mut editor = TextArea::default();
        editor.set_cursor_line_style(Style::default());
        editor.set_placeholder_text("Press Tab to switch here and start typing SQL...");
        editor.set_placeholder_style(Style::default().fg(Color::DarkGray));

        Self {
            sidebar_items,
            sidebar_state: state,
            editor,
            vim: Vim::new(vim::Mode::Normal),
            results_visible: false,
            query_result: None,
            focus: Focus::Sidebar,
            running: true,
            sidebar_width: config.sidebar_width,
            connected_db: None,
            message: None,
            profiles,
            connection: None,
            query_duration: None,
            show_help: false,
            keys: Keybindings::from_config(config.keybindings),
            label_to_profile,
            results_scroll_row: 0,
            results_scroll_col: 0,
            results_page: 0,
            results_has_more: false,
            results_query: None,
            results_area: Rect::default(),
            loading: None,
            spinner_tick: 0,
            bg_receiver: None,
            sidebar_filter: String::new(),
            sidebar_filtering: false,
            file_filter: String::new(),
            file_filtering: false,
            leader_active: false,
            show_sidebar: true,
            show_files: has_files,
            show_recent: false,
            recents: Recents::load(),
            recents_selected: 0,
            max_recents: config.max_recents,
            file_tree,
            file_tree_state,
            files_root,
            file_paths,
            query_language: QueryLanguage::default(),
            show_sql_preview: QueryLanguage::default().preview_by_default(),
        }
    }

    pub fn show_error(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            level: MessageLevel::Error,
        });
    }

    pub fn show_info(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            level: MessageLevel::Info,
        });
    }

    /// Returns the ordered list of currently visible focus targets.
    fn visible_panes(&self) -> Vec<Focus> {
        let mut panes = Vec::new();
        if self.show_sidebar { panes.push(Focus::Sidebar); }
        if self.show_files { panes.push(Focus::Files); }
        panes.push(Focus::QueryEditor); // always visible
        if self.results_visible { panes.push(Focus::Results); }
        if self.show_recent { panes.push(Focus::Recent); }
        panes
    }

    /// Cycles focus forward (or backward) through visible panes.
    fn next_visible_focus(&self, forward: bool) -> Focus {
        let panes = self.visible_panes();
        let current = panes.iter().position(|&f| f == self.focus).unwrap_or(0);
        if forward {
            panes[(current + 1) % panes.len()]
        } else {
            panes[(current + panes.len() - 1) % panes.len()]
        }
    }

    /// Toggles a pane's visibility. If the pane was focused, moves focus to
    /// the query editor.
    pub fn toggle_pane_visibility(&mut self, pane: Focus) {
        let (visible, is_focused) = match pane {
            Focus::Sidebar => (&mut self.show_sidebar, self.focus == Focus::Sidebar),
            Focus::Files => (&mut self.show_files, self.focus == Focus::Files),
            Focus::Recent => (&mut self.show_recent, self.focus == Focus::Recent),
            _ => return,
        };
        *visible = !*visible;
        if *visible {
            self.focus = pane;
        } else if is_focused {
            self.focus = Focus::QueryEditor;
        }
    }

    pub fn cycle_query_language(&mut self) {
        self.query_language = self.query_language.cycle();
        self.show_sql_preview = self.query_language.preview_by_default();
    }

    pub fn sql_preview(&self) -> String {
        let source = self.editor.lines().join("\n");
        self.query_language.transpile(&source)
            .unwrap_or_else(|e| format!("-- PRQL error\n{e}"))
    }

    pub fn format_query(&mut self) {
        let query: String = self.editor.lines().join("\n");
        if query.trim().is_empty() {
            return;
        }
        let formatted = match self.query_language {
            QueryLanguage::Sql => sqlformat::format(
                &query,
                &sqlformat::QueryParams::None,
                &sqlformat::FormatOptions {
                    indent: sqlformat::Indent::Spaces(2),
                    uppercase: Some(true),
                    lines_between_queries: 1,
                    ..Default::default()
                },
            ),
            QueryLanguage::Prql => {
                match prqlc::prql_to_pl(&query).and_then(|pl| prqlc::pl_to_prql(&pl)) {
                    Ok(s) => s,
                    Err(_) => return, // leave editor untouched on parse error
                }
            }
        };
        self.editor.select_all();
        self.editor.cut();
        self.editor.insert_str(&formatted);
    }

    pub fn execute_query(&mut self) {
        let source: String = self.editor.lines().join("\n");
        if source.trim().is_empty() {
            return;
        }
        let query = match self.query_language.transpile(source.trim()) {
            Ok(sql) => sql,
            Err(e) => {
                error!(error = %e, "transpilation failed");
                self.show_error(format!("Transpile error: {e}"));
                return;
            }
        };
        info!(query = query.trim(), "executing query");
        self.results_query = Some(query.trim().to_string());
        self.results_page = 0;
        self.results_scroll_row = 0;
        self.results_scroll_col = 0;
        self.run_paged_query();
    }

    fn run_paged_query(&mut self) {
        let Some(query) = &self.results_query else { return };
        let Some(mut conn) = self.connection.take() else {
            self.show_error("No database connected");
            return;
        };

        let is_select = query_is_select(query);
        let sql = if is_select {
            let offset = self.results_page * RESULTS_PAGE_SIZE;
            debug!(page = self.results_page, offset, "running paged query");
            let query_stripped = strip_trailing_semicolons(query);
            format!(
                "SELECT * FROM ({query_stripped}) AS _lazydb_q LIMIT {limit} OFFSET {offset}",
                limit = RESULTS_PAGE_SIZE + 1,
            )
        } else {
            debug!("running non-select query directly");
            query.clone()
        };

        let (tx, rx) = mpsc::channel();
        self.bg_receiver = Some(rx);
        self.loading = Some("Executing query…".into());
        self.spinner_tick = 0;

        std::thread::spawn(move || {
            let start = Instant::now();
            let result = match conn.execute_query(&sql) {
                Ok(mut result) => {
                    let duration = start.elapsed();
                    let has_more = is_select && result.rows.len() > RESULTS_PAGE_SIZE;
                    if has_more {
                        result.rows.truncate(RESULTS_PAGE_SIZE);
                    }
                    Ok((result, duration, has_more))
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(BgResult::Query { conn, result });
        });
    }

    pub fn results_next_page(&mut self) {
        if !self.results_has_more {
            return;
        }
        self.results_page += 1;
        self.results_scroll_row = 0;
        self.run_paged_query();
    }

    pub fn results_prev_page(&mut self) {
        if self.results_page == 0 {
            return;
        }
        self.results_page -= 1;
        self.results_scroll_row = 0;
        self.run_paged_query();
    }

    pub fn handle_event(&mut self) -> Result<()> {
        if self.poll_background() {
            // Background task just completed — return immediately so the
            // main loop re-renders without blocking on event::poll.
            return Ok(());
        }

        let poll_timeout = if self.loading.is_some() {
            Duration::from_millis(80)
        } else {
            Duration::from_secs(1)
        };

        if !event::poll(poll_timeout)? {
            if self.loading.is_some() {
                self.spinner_tick = self.spinner_tick.wrapping_add(1);
            }
            return Ok(());
        }

        let event = event::read()?;
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            // While loading, only allow quit
            if self.loading.is_some() {
                return Ok(());
            }

            // Dismiss message overlay
            if self.message.is_some() {
                self.message = None;
                return Ok(());
            }

            // Toggle help overlay
            if self.show_help {
                self.show_help = false;
                return Ok(());
            }

            let in_normal = !matches!(self.focus, Focus::QueryEditor)
                || self.vim.mode == vim::Mode::Normal;

            // Leader key dispatch: if leader is active, handle the action key
            if self.leader_active {
                self.leader_active = false;
                if let Event::Key(key) = &event {
                    self.handle_leader_action(key);
                }
                return Ok(());
            }

            // Activate leader mode on leader key press (only in normal-like contexts)
            if in_normal && self.keys.leader.matches(key) {
                self.leader_active = true;
                return Ok(());
            }

            // Global keybindings (only when not in editor insert mode)
            if in_normal {
                if self.keys.global.show_help.matches(key) {
                    self.show_help = true;
                    return Ok(());
                }
                if self.keys.global.execute_query.matches(key) {
                    self.execute_query();
                    return Ok(());
                }
                if self.keys.global.format_query.matches(key) {
                    self.format_query();
                    return Ok(());
                }
                if self.keys.global.next_pane.matches(key) {
                    self.focus = self.next_visible_focus(true);
                    return Ok(());
                }
                if self.keys.global.prev_pane.matches(key) {
                    self.focus = self.next_visible_focus(false);
                    return Ok(());
                }
            }

            match self.focus {
                Focus::QueryEditor => {
                    let input: Input = event.into();
                    match self.vim.transition(input, &mut self.editor) {
                        Transition::Mode(mode) if self.vim.mode != mode => {
                            self.vim = Vim::new(mode);
                        }
                        Transition::Nop | Transition::Mode(_) => {}
                        Transition::Pending(input) => {
                            self.vim = Vim::new(self.vim.mode).with_pending(input);
                        }
                    }
                }
                Focus::Sidebar if self.sidebar_filtering => self.handle_sidebar_filter_key(key),
                Focus::Sidebar => self.handle_sidebar_key(key),
                Focus::Results => self.handle_results_key(key),
                Focus::Files if self.file_filtering => self.handle_files_filter_key(key),
                Focus::Files => self.handle_files_key(key),
                Focus::Recent => self.handle_recent_key(key),
            }
        }
        Ok(())
    }

    /// Classifies the currently selected sidebar node.
    fn sidebar_node_kind(&self) -> SidebarNodeKind {
        let Some(selected) = self.sidebar_state.selected() else {
            return SidebarNodeKind::Other;
        };
        let flat = self.filtered_flat_nodes();
        let Some(node) = flat.get(selected) else {
            return SidebarNodeKind::Other;
        };

        if node.depth == 0 {
            return SidebarNodeKind::Connection;
        }

        // Walk backwards in the filtered list to find the immediate parent.
        // Using the filtered list (not unfiltered flat_index) is correct because
        // filtering force-expands collapsed ancestors, so flat_index values may
        // not correspond to positions in the unfiltered tree.
        let target_depth = node.depth;
        for ancestor in flat[..selected].iter().rev() {
            if ancestor.depth == target_depth - 1 {
                if ancestor.label == "Tables" || ancestor.label == "Views" {
                    return SidebarNodeKind::TableOrView;
                }
                break;
            }
        }

        SidebarNodeKind::Other
    }

    /// Returns the leader menu entries for the current focus context.
    pub fn leader_actions(&self) -> Vec<LeaderEntry> {
        let mut actions = Vec::new();
        match self.focus {
            Focus::QueryEditor => {
                actions.push(LeaderEntry { key: 'e', label: "Execute query" });
                actions.push(LeaderEntry { key: 'f', label: "Format query" });
                actions.push(LeaderEntry { key: 'l', label: "Switch language" });
                actions.push(LeaderEntry { key: 'p', label: "Toggle SQL preview" });
            }
            Focus::Sidebar => {
                let kind = self.sidebar_node_kind();
                match kind {
                    SidebarNodeKind::Connection => {
                        if self.connected_db.is_some() {
                            actions.push(LeaderEntry { key: 'd', label: "Disconnect" });
                        } else {
                            actions.push(LeaderEntry { key: 'o', label: "Connect" });
                        }
                    }
                    SidebarNodeKind::TableOrView => {
                        actions.push(LeaderEntry { key: 's', label: "Preview table" });
                    }
                    SidebarNodeKind::Other => {}
                }
                actions.push(LeaderEntry { key: 'e', label: "Execute query" });
            }
            Focus::Results => {
                actions.push(LeaderEntry { key: 'c', label: "Close results" });
                actions.push(LeaderEntry { key: 'e', label: "Execute query" });
            }
            Focus::Files => {
                actions.push(LeaderEntry { key: 'e', label: "Execute query" });
            }
            Focus::Recent => {
                actions.push(LeaderEntry { key: 'd', label: "Delete recent" });
                actions.push(LeaderEntry { key: 'e', label: "Execute query" });
            }
        }
        actions.push(LeaderEntry { key: '1', label: "Toggle connections" });
        actions.push(LeaderEntry { key: '2', label: "Toggle files" });
        actions.push(LeaderEntry { key: '3', label: "Toggle recent" });
        actions.push(LeaderEntry { key: 'h', label: "Help" });
        actions.push(LeaderEntry { key: 'q', label: "Quit" });
        actions
    }

    fn handle_leader_action(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        let KeyCode::Char(ch) = key.code else { return };

        // Only dispatch if the key is valid for current context
        if !self.leader_actions().iter().any(|a| a.key == ch) {
            return;
        }

        match ch {
            'e' => self.execute_query(),
            'f' => self.format_query(),
            'l' => self.cycle_query_language(),
            'p' => self.show_sql_preview = !self.show_sql_preview,
            'h' => self.show_help = true,
            's' => {
                if let Some(selected) = self.sidebar_state.selected() {
                    self.preview_table(selected);
                }
            }
            'o' => {
                // Connect: activate the selected connection node
                if let Some(selected) = self.sidebar_state.selected() {
                    let flat = self.filtered_flat_nodes();
                    if let Some(node) = flat.get(selected) {
                        if node.depth == 0 {
                            self.sidebar_filter.clear();
                            self.toggle_connection(node.flat_index);
                        }
                    }
                }
            }
            'd' => match self.focus {
                Focus::Sidebar => {
                    // Disconnect
                    if let Some(selected) = self.sidebar_state.selected() {
                        let flat = self.filtered_flat_nodes();
                        if let Some(node) = flat.get(selected) {
                            if node.depth == 0 {
                                self.sidebar_filter.clear();
                                self.toggle_connection(node.flat_index);
                            }
                        }
                    }
                }
                Focus::Recent => self.delete_selected_recent(),
                _ => {}
            }
            'c' => {
                self.results_visible = false;
                self.focus = Focus::QueryEditor;
            }
            '1' => self.toggle_pane_visibility(Focus::Sidebar),
            '2' => self.toggle_pane_visibility(Focus::Files),
            '3' => self.toggle_pane_visibility(Focus::Recent),
            'q' => self.running = false,
            _ => {}
        }
    }

    fn handle_sidebar_filter_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.sidebar_filtering = false;
                self.sidebar_filter.clear();
                // Reset selection
                if !self.sidebar_items.is_empty() {
                    self.sidebar_state.select(Some(0));
                }
            }
            KeyCode::Enter => {
                self.sidebar_filtering = false;
                // Keep filter active, selection stays
            }
            KeyCode::Backspace => {
                self.sidebar_filter.pop();
                if self.sidebar_filter.is_empty() {
                    self.sidebar_filtering = false;
                    if !self.sidebar_items.is_empty() {
                        self.sidebar_state.select(Some(0));
                    }
                } else {
                    let flat = self.filtered_flat_nodes();
                    if !flat.is_empty() {
                        self.sidebar_state.select(Some(0));
                    } else {
                        self.sidebar_state.select(None);
                    }
                }
            }
            KeyCode::Char(c) => {
                self.sidebar_filter.push(c);
                let flat = self.filtered_flat_nodes();
                if !flat.is_empty() {
                    self.sidebar_state.select(Some(0));
                } else {
                    self.sidebar_state.select(None);
                }
            }
            _ => {}
        }
    }

    /// Returns the flat nodes for the sidebar, respecting the current filter.
    pub fn filtered_flat_nodes(&self) -> Vec<crate::tree::FlatNode> {
        if self.sidebar_filter.is_empty() {
            TreeNode::flatten_all(&self.sidebar_items)
        } else {
            TreeNode::flatten_all_filtered(&self.sidebar_items, &self.sidebar_filter)
        }
    }

    /// Returns the flat nodes for the file tree, respecting the current filter.
    pub fn filtered_file_nodes(&self) -> Vec<crate::tree::FlatNode> {
        if self.file_filter.is_empty() {
            TreeNode::flatten_all(&self.file_tree)
        } else {
            TreeNode::flatten_all_filtered(&self.file_tree, &self.file_filter)
        }
    }

    fn handle_sidebar_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        // '/' activates search filter
        if key.code == KeyCode::Char('/') {
            self.sidebar_filtering = true;
            self.sidebar_filter.clear();
            return;
        }

        let flat = self.filtered_flat_nodes();
        let item_count = flat.len();
        let kb = &self.keys.sidebar;

        if kb.navigate_up.matches(key) {
            let selected = self.sidebar_state.selected().unwrap_or(0);
            if selected > 0 {
                self.sidebar_state.select(Some(selected - 1));
            }
        } else if kb.navigate_down.matches(key) {
            let selected = self.sidebar_state.selected().unwrap_or(0);
            if selected + 1 < item_count {
                self.sidebar_state.select(Some(selected + 1));
            }
        } else if kb.activate.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                if let Some(node) = flat.get(selected) {
                    let real_idx = node.flat_index;
                    let is_connection = node.depth == 0;
                    if is_connection {
                        self.sidebar_filter.clear();
                        self.toggle_connection(real_idx);
                    } else {
                        TreeNode::toggle_at_index(&mut self.sidebar_items, real_idx);
                    }
                }
            }
        } else if kb.expand.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                if let Some(node) = flat.get(selected) {
                    TreeNode::toggle_at_index(&mut self.sidebar_items, node.flat_index);
                }
            }
        } else if kb.collapse.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                if let Some(node) = flat.get(selected) {
                    TreeNode::collapse_at_index(&mut self.sidebar_items, node.flat_index);
                }
            }
        } else if kb.preview.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                self.preview_table(selected);
            }
        }
    }

    fn handle_results_key(&mut self, key: &crossterm::event::KeyEvent) {
        let kb = &self.keys.results;
        let result = self.query_result.as_ref();
        let row_count = result.map(|r| r.rows.len()).unwrap_or(0);
        let col_count = result.map(|r| r.columns.len()).unwrap_or(0);

        // Compute viewport capacity from last rendered area
        let area = self.results_area;
        let max_data_rows = if area.height > 5 {
            (area.height - 5) as usize
        } else {
            0
        };

        // Check if all columns fit at current scroll position
        let all_cols_visible = if let Some(r) = result {
            let inner_width = (area.width.saturating_sub(2)) as usize;
            let widths: Vec<usize> = r.columns.iter().enumerate().map(|(i, col)| {
                let max_data = r.rows.iter()
                    .map(|row| row.get(i).map(|v| v.to_string().len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                col.len().max(max_data).max(1)
            }).collect();
            // Check if all columns from scroll_col onward fit
            let mut used = 0;
            let mut fits = true;
            for (idx, &w) in widths.iter().enumerate().skip(self.results_scroll_col) {
                let needed = w + 3;
                if idx == self.results_scroll_col {
                    used += needed + 1;
                } else if used + needed <= inner_width {
                    used += needed;
                } else {
                    fits = false;
                    break;
                }
            }
            fits
        } else {
            true
        };

        if kb.close.matches(key) {
            self.results_visible = false;
            self.focus = Focus::QueryEditor;
        } else if kb.scroll_down.matches(key) {
            // Only scroll if rows overflow the viewport
            if row_count > max_data_rows && self.results_scroll_row + max_data_rows < row_count {
                self.results_scroll_row += 1;
            }
        } else if kb.scroll_up.matches(key) {
            self.results_scroll_row = self.results_scroll_row.saturating_sub(1);
        } else if kb.scroll_right.matches(key) {
            // Only scroll if there are clipped columns to the right
            if !all_cols_visible && self.results_scroll_col + 1 < col_count {
                self.results_scroll_col += 1;
            }
        } else if kb.scroll_left.matches(key) {
            self.results_scroll_col = self.results_scroll_col.saturating_sub(1);
        } else if kb.next_page.matches(key) {
            self.results_next_page();
        } else if kb.prev_page.matches(key) {
            self.results_prev_page();
        }
    }

    fn handle_files_filter_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.file_filtering = false;
                self.file_filter.clear();
                if !self.file_tree.is_empty() {
                    self.file_tree_state.select(Some(0));
                }
            }
            KeyCode::Enter => {
                self.file_filtering = false;
            }
            KeyCode::Backspace => {
                self.file_filter.pop();
                if self.file_filter.is_empty() {
                    self.file_filtering = false;
                    if !self.file_tree.is_empty() {
                        self.file_tree_state.select(Some(0));
                    }
                } else {
                    let flat = self.filtered_file_nodes();
                    if !flat.is_empty() {
                        self.file_tree_state.select(Some(0));
                    } else {
                        self.file_tree_state.select(None);
                    }
                }
            }
            KeyCode::Char(c) => {
                self.file_filter.push(c);
                let flat = self.filtered_file_nodes();
                if !flat.is_empty() {
                    self.file_tree_state.select(Some(0));
                } else {
                    self.file_tree_state.select(None);
                }
            }
            _ => {}
        }
    }

    fn handle_files_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Char('/') {
            self.file_filtering = true;
            self.file_filter.clear();
            return;
        }

        let flat = self.filtered_file_nodes();
        let item_count = flat.len();
        let kb = &self.keys.sidebar;

        if kb.navigate_up.matches(key) {
            let selected = self.file_tree_state.selected().unwrap_or(0);
            if selected > 0 {
                self.file_tree_state.select(Some(selected - 1));
            }
        } else if kb.navigate_down.matches(key) {
            let selected = self.file_tree_state.selected().unwrap_or(0);
            if selected + 1 < item_count {
                self.file_tree_state.select(Some(selected + 1));
            }
        } else if kb.activate.matches(key) {
            if let Some(selected) = self.file_tree_state.selected() {
                if let Some(node) = flat.get(selected) {
                    if node.has_children {
                        self.files_expand_or_toggle(node.flat_index);
                    } else {
                        self.files_open_file(selected);
                    }
                }
            }
        } else if kb.expand.matches(key) {
            if let Some(selected) = self.file_tree_state.selected() {
                if let Some(node) = flat.get(selected) {
                    if node.has_children && !node.expanded {
                        self.files_expand_or_toggle(node.flat_index);
                    }
                }
            }
        } else if kb.collapse.matches(key) {
            if let Some(selected) = self.file_tree_state.selected() {
                if let Some(node) = flat.get(selected) {
                    TreeNode::collapse_at_index(&mut self.file_tree, node.flat_index);
                    self.rebuild_file_paths();
                }
            }
        }
    }

    fn files_expand_or_toggle(&mut self, flat_index: usize) {
        // Lazy-populate if sentinel
        if crate::files::is_sentinel(&self.file_tree, flat_index) {
            crate::files::populate_children(
                &mut self.file_tree,
                flat_index,
                &self.file_paths,
            );
        }
        TreeNode::toggle_at_index(&mut self.file_tree, flat_index);
        self.rebuild_file_paths();
    }

    fn files_open_file(&mut self, flat_index: usize) {
        let Some(path) = self.file_paths.get(flat_index).cloned() else {
            return;
        };
        if !crate::files::is_text_file(&path) {
            self.message = Some(Message {
                text: format!("Not a text file: {}", path.display()),
                level: MessageLevel::Info,
            });
            return;
        }
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                self.editor.select_all();
                self.editor.cut();
                self.editor.insert_str(&contents);
                self.focus = Focus::QueryEditor;
                self.vim = crate::vim::Vim::new(crate::vim::Mode::Normal);
            }
            Err(e) => {
                self.show_error(format!("Failed to read {}: {e}", path.display()));
            }
        }
    }

    fn rebuild_file_paths(&mut self) {
        if let Some(ref root) = self.files_root {
            self.file_paths = crate::files::build_path_index(&self.file_tree, root);
        }
    }

    fn handle_recent_key(&mut self, key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Char('j') || key.code == KeyCode::Down {
            if !self.recents.entries.is_empty() {
                self.recents_selected = (self.recents_selected + 1)
                    .min(self.recents.entries.len() - 1);
                self.load_recent_into_results();
            }
        } else if key.code == KeyCode::Char('k') || key.code == KeyCode::Up {
            self.recents_selected = self.recents_selected.saturating_sub(1);
            if !self.recents.entries.is_empty() {
                self.load_recent_into_results();
            }
        } else if key.code == KeyCode::Enter {
            if let Some(entry) = self.recents.entries.get(self.recents_selected) {
                let lines: Vec<&str> = entry.query.lines().collect();
                self.editor = TextArea::new(lines.into_iter().map(String::from).collect());
                self.editor.set_cursor_line_style(Style::default());
                self.focus = Focus::QueryEditor;
                self.vim = Vim::new(vim::Mode::Normal);
                // Recent queries are always SQL (stored post-transpilation)
                self.query_language = QueryLanguage::Sql;
                self.show_sql_preview = false;
            }
        }
    }

    fn delete_selected_recent(&mut self) {
        if !self.recents.entries.is_empty() {
            self.recents.entries.remove(self.recents_selected);
            if self.recents_selected >= self.recents.entries.len() && self.recents_selected > 0 {
                self.recents_selected -= 1;
            }
            self.recents.save();
            self.load_recent_into_results();
        }
    }

    fn load_recent_into_results(&mut self) {
        if let Some(entry) = self.recents.entries.get(self.recents_selected) {
            self.query_result = entry.result.clone();
            self.query_duration = Some(Duration::from_millis(entry.duration_ms));
            self.results_has_more = false;
            self.results_scroll_row = 0;
            self.results_scroll_col = 0;
            self.results_page = 0;
            self.results_visible = entry.result.is_some();
        } else {
            self.query_result = None;
            self.results_visible = false;
        }
    }

    fn toggle_connection(&mut self, flat_index: usize) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let Some(node) = flat.get(flat_index) else { return };
        let label = node.label.clone();

        if self.connected_db.as_ref() == Some(&label) {
            // Disconnect: drop connection, clear schema, collapse
            info!(label = %label, "disconnecting from database");
            self.connection = None;
            self.connected_db = None;
            self.clear_schema(&label);
            TreeNode::collapse_at_index(&mut self.sidebar_items, flat_index);
        } else {
            // Collapse previously connected db
            if let Some(prev) = &self.connected_db {
                self.connection = None;
                let prev = prev.clone();
                self.clear_schema(&prev);
                for node in self.sidebar_items.iter_mut() {
                    if node.label == prev && node.expanded {
                        node.expanded = false;
                        break;
                    }
                }
            }

            // Look up profile and connect in background
            let profile_key = self.label_to_profile.get(&label).cloned();
            let profile = profile_key
                .as_ref()
                .and_then(|k| self.profiles.connections.get(k))
                .cloned();

            if let Some(profile) = profile {
                info!(label = %label, "connecting to database");
                let (tx, rx) = mpsc::channel();
                self.bg_receiver = Some(rx);
                self.loading = Some("Connecting…".into());
                self.spinner_tick = 0;

                let label_clone = label.clone();
                std::thread::spawn(move || {
                    let result = Self::connect_profile(&profile);
                    let result = match result {
                        Ok(mut db) => {
                            let schema = db.schema_tree().unwrap_or_default();
                            Ok((db, schema))
                        }
                        Err(e) => Err(e),
                    };
                    let _ = tx.send(BgResult::Connected {
                        label: label_clone,
                        result,
                    });
                });
            }
        }
    }

    fn connect_profile(profile: &Connection) -> Result<Box<dyn Database>, String> {
        profile.connect()
    }

    fn preview_table(&mut self, flat_pos: usize) {
        let flat = self.filtered_flat_nodes();
        let Some(node) = flat.get(flat_pos) else { return };

        // Collect ancestors by walking backwards in the filtered list.
        // The filtered list always includes ancestors of matching nodes, so this
        // works correctly even when filtering force-expands collapsed folders.
        let mut ancestors: Vec<&str> = Vec::new();
        let mut target_depth = node.depth;
        for ancestor in flat[..flat_pos].iter().rev() {
            if ancestor.depth < target_depth {
                ancestors.push(&ancestor.label);
                target_depth = ancestor.depth;
                if target_depth == 0 {
                    break;
                }
            }
        }

        // Ancestors are in reverse order (innermost first): [Tables/Views, schema, database, connection]
        // Check that the immediate parent is "Tables" or "Views"
        if !matches!(ancestors.first().map(|s| s.as_ref()), Some("Tables" | "Views")) {
            return;
        }

        // Build fully qualified name based on connection type.
        // ancestors = [Tables/Views, ...intermediate levels..., connection]
        let table_name = node.label.as_str();
        let connection_label = ancestors.last().copied().unwrap_or(table_name);
        let conn_type = self.label_to_profile.get(connection_label)
            .and_then(|k| self.profiles.connections.get(k))
            .map(|c| c.type_name());

        let qualified_name = match conn_type {
            // Snowflake: database.schema.table
            Some("snowflake") => {
                let schema_name = ancestors.get(1).unwrap_or(&table_name);
                let db_name = ancestors.get(2).unwrap_or(schema_name);
                format!("{db_name}.{schema_name}.{table_name}")
            }
            // DuckDB: schema.table
            Some("duckdb") => {
                let schema_name = ancestors.get(1).unwrap_or(&table_name);
                format!("{schema_name}.{table_name}")
            }
            // PostgreSQL: schema.table
            Some("postgres") => {
                let schema_name = ancestors.get(1).unwrap_or(&table_name);
                format!("{schema_name}.{table_name}")
            }
            // ClickHouse: just table (schema tree has no schema level)
            Some("clickhouse") => table_name.to_string(),
            // Fallback: use whatever ancestors are available
            _ => {
                let schema_name = ancestors.get(1).unwrap_or(&table_name);
                format!("{schema_name}.{table_name}")
            }
        };
        let query = format!("SELECT * FROM {qualified_name} LIMIT 10");

        // Clear editor and insert the query
        self.editor.select_all();
        self.editor.cut();
        self.editor.insert_str(&query);
        self.focus = Focus::QueryEditor;
        self.vim = Vim::new(vim::Mode::Normal);
    }

    fn poll_background(&mut self) -> bool {
        let result = match &self.bg_receiver {
            Some(rx) => match rx.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => return false,
                Err(mpsc::TryRecvError::Disconnected) => {
                    error!("background task channel disconnected unexpectedly");
                    self.loading = None;
                    self.bg_receiver = None;
                    self.show_error("Background task failed unexpectedly");
                    return true;
                }
            },
            None => return false,
        };

        self.loading = None;
        self.bg_receiver = None;

        match result {
            BgResult::Connected { label, result } => match result {
                Ok((db, schema)) => {
                    info!(label = %label, schema_nodes = schema.len(), "connection established successfully");
                    self.populate_schema(&label, schema);
                    self.connection = Some(db);
                    self.connected_db = Some(label.clone());
                    // Expand the connection node
                    let new_flat = TreeNode::flatten_all(&self.sidebar_items);
                    if let Some(new_idx) = new_flat.iter().position(|n| n.label == label) {
                        TreeNode::toggle_at_index(&mut self.sidebar_items, new_idx);
                        self.sidebar_state.select(Some(new_idx));
                    }
                }
                Err(e) => {
                    error!(label = %label, error = %e, "connection failed");
                    self.show_error(format!("Connection failed: {e}"));
                }
            },
            BgResult::Query { conn, result } => {
                self.connection = Some(conn);
                match result {
                    Ok((query_result, duration, has_more)) => {
                        info!(
                            rows = query_result.rows.len(),
                            columns = query_result.columns.len(),
                            duration_ms = duration.as_millis(),
                            has_more,
                            "query completed"
                        );
                        self.query_duration = Some(duration);
                        self.results_has_more = has_more;
                        self.query_result = Some(query_result);
                        self.results_visible = true;
                        self.focus = Focus::Results;
                        self.save_recent_entry(None);
                        // Refresh schema after DDL/DML that may have changed it
                        if !has_more {
                            if let Some(q) = &self.results_query {
                                if !query_is_select(q) {
                                    self.refresh_schema();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "query failed");
                        self.save_recent_entry(Some(e.clone()));
                        self.show_error(format!("Query error: {e}"));
                    }
                }
            }
        }
        true
    }

    fn save_recent_entry(&mut self, error: Option<String>) {
        use std::time::{SystemTime, UNIX_EPOCH};

        let Some(query) = &self.results_query else { return };

        let entry = RecentEntry {
            query: query.clone(),
            connection: self.connected_db.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            duration_ms: self.query_duration
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            result: if error.is_none() { self.query_result.clone() } else { None },
            error,
        };
        self.recents.add(entry, self.max_recents);
        self.recents.save();
    }

    fn refresh_schema(&mut self) {
        let Some(label) = self.connected_db.clone() else { return };
        let Some(conn) = self.connection.as_mut() else { return };
        debug!(label = %label, "refreshing schema tree");
        match conn.schema_tree() {
            Ok(tree) => {
                debug!(label = %label, nodes = tree.len(), "schema tree refreshed");
                self.populate_schema(&label, tree);
            }
            Err(e) => {
                debug!(label = %label, error = %e, "schema refresh failed (silent)");
            }
        }
    }

    fn populate_schema(&mut self, connection_label: &str, schema_nodes: Vec<SchemaNode>) {
        fn to_tree(node: SchemaNode) -> TreeNode {
            if node.children.is_empty() {
                TreeNode::leaf(&node.label)
            } else {
                TreeNode::folder(&node.label, node.children.into_iter().map(to_tree).collect())
            }
        }

        for node in self.sidebar_items.iter_mut() {
            if node.label == connection_label {
                node.children = schema_nodes.into_iter().map(to_tree).collect();
                break;
            }
        }
    }

    fn clear_schema(&mut self, connection_label: &str) {
        for node in self.sidebar_items.iter_mut() {
            if node.label == connection_label {
                node.children.clear();
                break;
            }
        }
    }
}

#[cfg(test)]
impl<'a> App<'a> {
    fn set_bg_receiver(&mut self, rx: mpsc::Receiver<BgResult>) {
        self.bg_receiver = Some(rx);
        self.loading = Some("test".into());
    }

    fn handle_leader_key_press(&mut self) {
        // Simulate pressing the leader key in a normal-mode context
        let in_normal = self.focus != Focus::QueryEditor
            || self.vim.mode == vim::Mode::Normal;
        if in_normal {
            self.leader_active = true;
        }
    }
}

/// Strips trailing semicolons (and surrounding whitespace) from a SQL query.
/// Required before embedding a query as a subquery, since semicolons are
/// statement terminators and are invalid inside subquery expressions.
fn strip_trailing_semicolons(sql: &str) -> &str {
    let mut s = sql.trim_end();
    while s.ends_with(';') {
        s = s[..s.len() - 1].trim_end();
    }
    s
}

/// Returns true if the query is a SELECT-like statement that returns rows
/// and can be wrapped in a paging subquery.
fn query_is_select(sql: &str) -> bool {
    let trimmed = sql.trim_start();
    // Strip leading CTEs: WITH ... SELECT
    let s = if trimmed.len() >= 4 && trimmed[..4].eq_ignore_ascii_case("with") {
        trimmed
    } else {
        trimmed
    };
    let upper_start: String = s.chars().take(10).collect::<String>().to_ascii_uppercase();
    upper_start.starts_with("SELECT")
        || upper_start.starts_with("WITH")
        || upper_start.starts_with("TABLE ")
        || upper_start.starts_with("VALUES")
}

fn build_sidebar_tree(profiles: &Profiles) -> (Vec<TreeNode>, BTreeMap<String, String>) {
    let mut label_map = BTreeMap::new();
    let nodes = profiles
        .connections
        .iter()
        .map(|(name, conn)| {
            let label = format!("{} ({})", name, conn.type_name());
            label_map.insert(label.clone(), name.clone());
            TreeNode::connection(&label, vec![])
        })
        .collect();
    (nodes, label_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DuckDbConnection, PostgresConnection};
    use crate::db::{MockDatabase, SchemaNode};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn test_app() -> App<'static> {
        App::new(AppConfig::default(), Profiles::default(), None)
    }

    fn test_profiles() -> Profiles {
        let mut connections = BTreeMap::new();
        connections.insert(
            "testdb".into(),
            Connection::DuckDb(DuckDbConnection { path: ":memory:".into() }),
        );
        connections.insert(
            "pgdb".into(),
            Connection::Postgres(PostgresConnection {
                host: "localhost".into(),
                port: 5432,
                user: "test".into(),
                password: None,
                database: "testdb".into(),
                schema: None,
            }),
        );
        Profiles { connections }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // --- Initialization ---

    #[test]
    fn new_empty_profiles() {
        let app = test_app();
        assert!(app.sidebar_items.is_empty());
        assert_eq!(app.sidebar_state.selected(), None);
        assert_eq!(app.focus, Focus::Sidebar);
        assert!(app.running);
    }

    #[test]
    fn new_with_profiles() {
        let app = App::new(AppConfig::default(), test_profiles(), None);
        assert_eq!(app.sidebar_items.len(), 2);
        assert_eq!(app.sidebar_state.selected(), Some(0));
    }

    #[test]
    fn build_sidebar_tree_labels() {
        let profiles = test_profiles();
        let (nodes, label_map) = build_sidebar_tree(&profiles);
        // BTreeMap iterates in sorted order: pgdb, testdb
        assert_eq!(nodes[0].label, "pgdb (postgres)");
        assert_eq!(nodes[1].label, "testdb (duckdb)");
        assert_eq!(label_map.get("pgdb (postgres)"), Some(&"pgdb".to_string()));
        assert_eq!(label_map.get("testdb (duckdb)"), Some(&"testdb".to_string()));
    }

    // --- Focus cycling ---

    #[test]
    fn tab_cycles_focus() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_visible = true;

        assert_eq!(app.focus, Focus::Sidebar);

        // Sidebar -> QueryEditor
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::QueryEditor);

        // QueryEditor -> Results (visible)
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Results);

        // Results -> Sidebar (wraps)
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn tab_skips_results_when_hidden() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_visible = false;
        app.focus = Focus::QueryEditor;

        // QueryEditor -> Sidebar (results hidden, wraps)
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn shift_tab_cycles_reverse() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_visible = true;
        app.focus = Focus::Sidebar;

        // prev_pane: Sidebar -> Results (when visible, wraps backwards)
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::Results);
    }

    // --- Sidebar key handling ---

    #[test]
    fn sidebar_navigate_down() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert_eq!(app.sidebar_state.selected(), Some(0));
        app.handle_sidebar_key(&key(KeyCode::Char('j')));
        assert_eq!(app.sidebar_state.selected(), Some(1));
    }

    #[test]
    fn sidebar_navigate_up() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_state.select(Some(1));
        app.handle_sidebar_key(&key(KeyCode::Char('k')));
        assert_eq!(app.sidebar_state.selected(), Some(0));
    }

    #[test]
    fn sidebar_navigate_down_at_bottom() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_state.select(Some(1)); // last item (2 profiles)
        app.handle_sidebar_key(&key(KeyCode::Char('j')));
        assert_eq!(app.sidebar_state.selected(), Some(1)); // unchanged
    }

    #[test]
    fn sidebar_navigate_up_at_top() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert_eq!(app.sidebar_state.selected(), Some(0));
        app.handle_sidebar_key(&key(KeyCode::Char('k')));
        assert_eq!(app.sidebar_state.selected(), Some(0)); // unchanged
    }

    #[test]
    fn sidebar_q_does_not_quit() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(app.running);
        app.handle_sidebar_key(&key(KeyCode::Char('q')));
        assert!(app.running);
    }

    #[test]
    fn sidebar_slash_starts_filter() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(!app.sidebar_filtering);
        app.handle_sidebar_key(&key(KeyCode::Char('/')));
        assert!(app.sidebar_filtering);
        assert!(app.sidebar_filter.is_empty());
    }

    // --- Sidebar filter key handling ---

    #[test]
    fn filter_char_appends() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_filtering = true;
        app.handle_sidebar_filter_key(&key(KeyCode::Char('t')));
        assert_eq!(app.sidebar_filter, "t");
        app.handle_sidebar_filter_key(&key(KeyCode::Char('e')));
        assert_eq!(app.sidebar_filter, "te");
    }

    #[test]
    fn filter_backspace_pops() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_filtering = true;
        app.sidebar_filter = "te".into();
        app.handle_sidebar_filter_key(&key(KeyCode::Backspace));
        assert_eq!(app.sidebar_filter, "t");
    }

    #[test]
    fn filter_backspace_on_empty_exits() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_filtering = true;
        app.sidebar_filter = "x".into();
        app.handle_sidebar_filter_key(&key(KeyCode::Backspace));
        // After popping 'x', filter is empty -> exits filtering
        assert!(!app.sidebar_filtering);
        assert!(app.sidebar_filter.is_empty());
    }

    #[test]
    fn filter_esc_clears_and_exits() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_filtering = true;
        app.sidebar_filter = "test".into();
        app.handle_sidebar_filter_key(&key(KeyCode::Esc));
        assert!(!app.sidebar_filtering);
        assert!(app.sidebar_filter.is_empty());
    }

    #[test]
    fn filter_enter_keeps_filter_exits_mode() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.sidebar_filtering = true;
        app.sidebar_filter = "pg".into();
        app.handle_sidebar_filter_key(&key(KeyCode::Enter));
        assert!(!app.sidebar_filtering);
        assert_eq!(app.sidebar_filter, "pg"); // filter kept
    }

    // --- Files filter key handling ---

    fn app_with_file_tree() -> App<'static> {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.file_tree = vec![
            TreeNode::folder("src", vec![
                TreeNode::leaf("main.sql"),
                TreeNode::leaf("queries.sql"),
            ]),
            TreeNode::leaf("README.md"),
        ];
        app.file_tree_state.select(Some(0));
        app
    }

    #[test]
    fn files_slash_starts_filter() {
        let mut app = app_with_file_tree();
        app.focus = Focus::Files;
        assert!(!app.file_filtering);
        app.handle_files_key(&key(KeyCode::Char('/')));
        assert!(app.file_filtering);
        assert!(app.file_filter.is_empty());
    }

    #[test]
    fn files_filter_char_appends() {
        let mut app = app_with_file_tree();
        app.file_filtering = true;
        app.handle_files_filter_key(&key(KeyCode::Char('s')));
        assert_eq!(app.file_filter, "s");
        app.handle_files_filter_key(&key(KeyCode::Char('q')));
        assert_eq!(app.file_filter, "sq");
    }

    #[test]
    fn files_filter_backspace_pops() {
        let mut app = app_with_file_tree();
        app.file_filtering = true;
        app.file_filter = "sq".into();
        app.handle_files_filter_key(&key(KeyCode::Backspace));
        assert_eq!(app.file_filter, "s");
    }

    #[test]
    fn files_filter_backspace_on_empty_exits() {
        let mut app = app_with_file_tree();
        app.file_filtering = true;
        app.file_filter = "x".into();
        app.handle_files_filter_key(&key(KeyCode::Backspace));
        assert!(!app.file_filtering);
        assert!(app.file_filter.is_empty());
    }

    #[test]
    fn files_filter_esc_clears_and_exits() {
        let mut app = app_with_file_tree();
        app.file_filtering = true;
        app.file_filter = "sql".into();
        app.handle_files_filter_key(&key(KeyCode::Esc));
        assert!(!app.file_filtering);
        assert!(app.file_filter.is_empty());
    }

    #[test]
    fn files_filter_enter_keeps_filter_exits_mode() {
        let mut app = app_with_file_tree();
        app.file_filtering = true;
        app.file_filter = "sql".into();
        app.handle_files_filter_key(&key(KeyCode::Enter));
        assert!(!app.file_filtering);
        assert_eq!(app.file_filter, "sql");
    }

    #[test]
    fn files_filtered_nodes_match_substring() {
        let mut app = app_with_file_tree();
        app.file_filter = "sql".into();
        let nodes = app.filtered_file_nodes();
        // Matching leaves appear; ancestors of matches also appear (same as sidebar behaviour)
        assert!(nodes.iter().any(|n| n.label == "main.sql"));
        assert!(nodes.iter().any(|n| n.label == "queries.sql"));
        assert!(nodes.iter().any(|n| n.label == "src")); // ancestor of matching children
        assert!(!nodes.iter().any(|n| n.label == "README.md")); // no match, no ancestor path
    }

    // --- Results key handling ---

    #[test]
    fn results_scroll_down() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.query_result = Some(QueryResult {
            columns: vec!["id".into()],
            rows: (0..20).map(|i| vec![db::Value::Int(i)]).collect(),
        });
        app.results_area = Rect::new(0, 0, 80, 15); // height 15 -> max_data_rows = 10
        app.handle_results_key(&key(KeyCode::Char('j')));
        assert_eq!(app.results_scroll_row, 1);
    }

    #[test]
    fn results_scroll_down_at_bottom() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.query_result = Some(QueryResult {
            columns: vec!["id".into()],
            rows: (0..5).map(|i| vec![db::Value::Int(i)]).collect(),
        });
        app.results_area = Rect::new(0, 0, 80, 15); // max_data_rows=10, but only 5 rows
        app.handle_results_key(&key(KeyCode::Char('j')));
        assert_eq!(app.results_scroll_row, 0); // no scroll needed
    }

    #[test]
    fn results_scroll_up() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_scroll_row = 3;
        app.handle_results_key(&key(KeyCode::Char('k')));
        assert_eq!(app.results_scroll_row, 2);
    }

    #[test]
    fn results_scroll_up_at_zero() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_scroll_row = 0;
        app.handle_results_key(&key(KeyCode::Char('k')));
        assert_eq!(app.results_scroll_row, 0);
    }

    #[test]
    fn results_close() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_visible = true;
        app.focus = Focus::Results;
        app.handle_results_key(&key(KeyCode::Char('c')));
        assert!(!app.results_visible);
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn results_q_does_not_quit() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Results;
        app.handle_results_key(&key(KeyCode::Char('q')));
        assert!(app.running);
    }

    // --- Pagination ---

    #[test]
    fn next_page_noop_when_no_more() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_has_more = false;
        app.results_page = 0;
        app.results_next_page();
        assert_eq!(app.results_page, 0);
    }

    #[test]
    fn prev_page_noop_at_zero() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.results_page = 0;
        app.results_prev_page();
        assert_eq!(app.results_page, 0);
    }

    // --- Schema mutation ---

    #[test]
    fn populate_schema_adds_children() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let label = &app.sidebar_items[0].label.clone();
        let schema = vec![
            SchemaNode::group("Tables", vec![
                SchemaNode::leaf("users"),
                SchemaNode::leaf("orders"),
            ]),
        ];
        app.populate_schema(label, schema);
        assert_eq!(app.sidebar_items[0].children.len(), 1);
        assert_eq!(app.sidebar_items[0].children[0].label, "Tables");
        assert_eq!(app.sidebar_items[0].children[0].children.len(), 2);
    }

    #[test]
    fn clear_schema_removes_children() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let label = app.sidebar_items[0].label.clone();
        let schema = vec![SchemaNode::leaf("Tables")];
        app.populate_schema(&label, schema);
        assert!(!app.sidebar_items[0].children.is_empty());
        app.clear_schema(&label);
        assert!(app.sidebar_items[0].children.is_empty());
    }

    // --- poll_background ---

    #[test]
    fn poll_connected_success() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let label = app.sidebar_items[0].label.clone();

        let (tx, rx) = mpsc::channel();
        app.set_bg_receiver(rx);

        let mock = MockDatabase::new().with_schema(vec![
            SchemaNode::group("Tables", vec![SchemaNode::leaf("users")]),
        ]);
        tx.send(BgResult::Connected {
            label: label.clone(),
            result: Ok((Box::new(mock), vec![
                SchemaNode::group("Tables", vec![SchemaNode::leaf("users")]),
            ])),
        }).unwrap();

        app.poll_background();
        assert_eq!(app.connected_db, Some(label));
        assert!(app.connection.is_some());
        assert!(app.loading.is_none());
    }

    #[test]
    fn poll_connected_failure() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let label = app.sidebar_items[0].label.clone();

        let (tx, rx) = mpsc::channel();
        app.set_bg_receiver(rx);

        tx.send(BgResult::Connected {
            label,
            result: Err("connection refused".into()),
        }).unwrap();

        app.poll_background();
        assert!(app.connected_db.is_none());
        assert!(app.message.is_some());
        assert_eq!(app.message.as_ref().unwrap().level, MessageLevel::Error);
    }

    #[test]
    fn poll_query_success() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let (tx, rx) = mpsc::channel();
        app.set_bg_receiver(rx);

        let mock = MockDatabase::new();
        let result = QueryResult {
            columns: vec!["id".into()],
            rows: vec![vec![db::Value::Int(1)]],
        };
        tx.send(BgResult::Query {
            conn: Box::new(mock),
            result: Ok((result, Duration::from_millis(42), false)),
        }).unwrap();

        app.poll_background();
        assert!(app.query_result.is_some());
        assert!(app.results_visible);
        assert_eq!(app.focus, Focus::Results);
        assert!(!app.results_has_more);
    }

    #[test]
    fn poll_disconnected_channel() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        let (tx, rx) = mpsc::channel();
        app.set_bg_receiver(rx);
        drop(tx); // disconnect the channel

        app.poll_background();
        assert!(app.loading.is_none());
        assert!(app.message.is_some());
    }

    // --- Message helpers ---

    #[test]
    fn show_error_sets_message() {
        let mut app = test_app();
        app.show_error("something broke");
        assert!(app.message.is_some());
        assert_eq!(app.message.as_ref().unwrap().text, "something broke");
        assert_eq!(app.message.as_ref().unwrap().level, MessageLevel::Error);
    }

    #[test]
    fn show_info_sets_message() {
        let mut app = test_app();
        app.show_info("all good");
        assert!(app.message.is_some());
        assert_eq!(app.message.as_ref().unwrap().level, MessageLevel::Info);
    }

    // --- Leader key ---

    #[test]
    fn leader_key_activates() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(!app.leader_active);
        // Space is the default leader key, sidebar is default focus (normal context)
        app.handle_leader_key_press();
        assert!(app.leader_active);
    }

    #[test]
    fn leader_actions_query_editor() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 'e'), "execute");
        assert!(actions.iter().any(|a| a.key == 'f'), "format");
        assert!(actions.iter().any(|a| a.key == 'l'), "switch language");
        assert!(actions.iter().any(|a| a.key == 'h'), "help");
        assert!(!actions.iter().any(|a| a.key == 's'), "no preview");
        assert!(!actions.iter().any(|a| a.key == 'c'), "no close");
    }

    #[test]
    fn query_language_cycles() {
        assert_eq!(QueryLanguage::Sql.cycle(), QueryLanguage::Prql);
        assert_eq!(QueryLanguage::Prql.cycle(), QueryLanguage::Sql);
    }

    #[test]
    fn query_language_default_is_sql() {
        let app = App::new(AppConfig::default(), test_profiles(), None);
        assert_eq!(app.query_language, QueryLanguage::Sql);
    }

    #[test]
    fn sql_preview_hidden_by_default() {
        let app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(!app.show_sql_preview);
    }

    #[test]
    fn switching_to_prql_shows_preview() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.handle_leader_action(&key(KeyCode::Char('l')));
        assert_eq!(app.query_language, QueryLanguage::Prql);
        assert!(app.show_sql_preview);
    }

    #[test]
    fn switching_back_to_sql_hides_preview() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.handle_leader_action(&key(KeyCode::Char('l'))); // → PRQL
        app.handle_leader_action(&key(KeyCode::Char('l'))); // → SQL
        assert_eq!(app.query_language, QueryLanguage::Sql);
        assert!(!app.show_sql_preview);
    }

    #[test]
    fn toggle_preview_keybind() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        assert!(!app.show_sql_preview);
        app.handle_leader_action(&key(KeyCode::Char('p')));
        assert!(app.show_sql_preview);
        app.handle_leader_action(&key(KeyCode::Char('p')));
        assert!(!app.show_sql_preview);
    }

    #[test]
    fn sql_preview_returns_source_for_sql() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.editor.insert_str("SELECT 1");
        assert_eq!(app.sql_preview(), "SELECT 1");
    }

    #[test]
    fn sql_preview_transpiles_prql() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.handle_leader_action(&key(KeyCode::Char('l'))); // → PRQL
        app.editor.insert_str("from employees | select {name}");
        let preview = app.sql_preview();
        assert!(!preview.contains("-- PRQL error"), "expected valid SQL, got: {preview}");
        assert!(preview.to_lowercase().contains("select"));
    }

    #[test]
    fn sql_preview_shows_error_for_invalid_prql() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.handle_leader_action(&key(KeyCode::Char('l'))); // → PRQL
        app.editor.insert_str("this is not valid prql !!!");
        let preview = app.sql_preview();
        assert!(preview.contains("-- PRQL error"), "expected error comment, got: {preview}");
    }

    #[test]
    fn leader_action_l_cycles_language() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        assert_eq!(app.query_language, QueryLanguage::Sql);
        app.handle_leader_action(&key(KeyCode::Char('l')));
        assert_eq!(app.query_language, QueryLanguage::Prql);
        app.handle_leader_action(&key(KeyCode::Char('l')));
        assert_eq!(app.query_language, QueryLanguage::Sql);
    }

    #[test]
    fn switch_language_not_available_outside_query_editor() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        let actions = app.leader_actions();
        assert!(!actions.iter().any(|a| a.key == 'l'), "no switch language in sidebar");
        assert!(!actions.iter().any(|a| a.key == 'p'), "no preview toggle in sidebar");
    }

    #[test]
    fn leader_actions_sidebar_on_connection() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        // Default selection is index 0, which is a connection node (depth 0)
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 'o'), "connect");
        assert!(!actions.iter().any(|a| a.key == 's'), "no preview on connection");
        assert!(actions.iter().any(|a| a.key == 'e'), "execute");
        assert!(actions.iter().any(|a| a.key == 'h'), "help");
    }

    #[test]
    fn leader_actions_sidebar_on_table() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        // Populate schema so we have table nodes
        let label = app.sidebar_items[0].label.clone();
        app.populate_schema(&label, vec![
            SchemaNode::group("Tables", vec![
                SchemaNode::leaf("users"),
            ]),
        ]);
        // Expand the connection and Tables folder to make "users" visible
        app.sidebar_items[0].expanded = true;
        app.sidebar_items[0].children[0].expanded = true;
        // "users" should be at flat index 2 (connection=0, Tables=1, users=2)
        app.sidebar_state.select(Some(2));
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 's'), "preview on table");
        assert!(!actions.iter().any(|a| a.key == 'o'), "no connect on table");
        assert!(!actions.iter().any(|a| a.key == 'd'), "no disconnect on table");
    }

    #[test]
    fn leader_s_with_filter_shows_table_when_parent_collapsed() {
        // Regression: when a filter force-expands a collapsed Tables folder to show
        // a matching table, leader+s must still insert the preview query.
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        let label = app.sidebar_items[0].label.clone();
        app.populate_schema(&label, vec![
            SchemaNode::group("Tables", vec![
                SchemaNode::leaf("users"),
            ]),
        ]);
        // Connection is expanded but Tables is NOT expanded (collapsed)
        app.sidebar_items[0].expanded = true;
        // app.sidebar_items[0].children[0].expanded stays false

        // Apply a filter that matches "users" — filtered view force-expands Tables
        app.sidebar_filter = "users".into();
        app.sidebar_filtering = false;

        // The filtered list should show: Connection(0), Tables(1), users(2)
        let flat = app.filtered_flat_nodes();
        let users_pos = flat.iter().position(|n| n.label == "users").expect("users in filtered list");
        app.sidebar_state.select(Some(users_pos));

        // leader_actions must include 's'
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 's'), "preview action should be available with filter active");

        // leader+s must insert the preview query into the editor
        app.leader_active = true;
        app.handle_leader_action(&key(KeyCode::Char('s')));
        let editor_text: String = app.editor.lines().join("\n");
        assert!(editor_text.contains("users"), "expected preview query with 'users', got: {editor_text:?}");
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn leader_actions_sidebar_on_folder() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        let label = app.sidebar_items[0].label.clone();
        app.populate_schema(&label, vec![
            SchemaNode::group("Tables", vec![
                SchemaNode::leaf("users"),
            ]),
        ]);
        app.sidebar_items[0].expanded = true;
        // Select the "Tables" folder (flat index 1)
        app.sidebar_state.select(Some(1));
        let actions = app.leader_actions();
        assert!(!actions.iter().any(|a| a.key == 's'), "no preview on folder");
        assert!(!actions.iter().any(|a| a.key == 'o'), "no connect on folder");
        assert!(actions.iter().any(|a| a.key == 'e'), "execute always available");
        assert!(actions.iter().any(|a| a.key == 'h'), "help always available");
    }

    #[test]
    fn leader_actions_sidebar_disconnect_when_connected() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        app.connected_db = Some(app.sidebar_items[0].label.clone());
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 'd'), "disconnect when connected");
        assert!(!actions.iter().any(|a| a.key == 'o'), "no connect when already connected");
    }

    #[test]
    fn leader_actions_results() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Results;
        let actions = app.leader_actions();
        assert!(actions.iter().any(|a| a.key == 'c'), "close");
        assert!(actions.iter().any(|a| a.key == 'e'), "execute");
        assert!(actions.iter().any(|a| a.key == 'h'), "help");
        assert!(!actions.iter().any(|a| a.key == 'f'), "no format");
    }

    #[test]
    fn leader_action_format() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.editor.insert_str("select * from foo");
        app.handle_leader_action(&key(KeyCode::Char('f')));
        let text: String = app.editor.lines().join("\n");
        assert!(text.contains("SELECT"));
    }

    #[test]
    fn leader_action_help() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.handle_leader_action(&key(KeyCode::Char('h')));
        assert!(app.show_help);
    }

    #[test]
    fn leader_action_quit() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(app.running);
        app.handle_leader_action(&key(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn leader_q_quits_from_sidebar() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        assert!(app.running);
        app.leader_active = true;
        app.handle_leader_action(&key(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn leader_q_quits_from_results() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Results;
        assert!(app.running);
        app.leader_active = true;
        app.handle_leader_action(&key(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn leader_actions_include_quit_always() {
        for focus in [Focus::Sidebar, Focus::QueryEditor, Focus::Results] {
            let mut app = App::new(AppConfig::default(), test_profiles(), None);
            app.focus = focus;
            let actions = app.leader_actions();
            assert!(
                actions.iter().any(|a| a.key == 'q'),
                "quit missing from leader_actions for focus {focus:?}"
            );
        }
    }

    #[test]
    fn leader_action_close_results() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Results;
        app.results_visible = true;
        app.handle_leader_action(&key(KeyCode::Char('c')));
        assert!(!app.results_visible);
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn leader_ignores_invalid_action_for_context() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        // 'f' (format) is not available in sidebar context
        app.handle_leader_action(&key(KeyCode::Char('f')));
        // Should be a no-op — query text unchanged
        let text: String = app.editor.lines().join("\n");
        assert!(text.is_empty());
    }

    #[test]
    fn leader_not_active_in_insert_mode() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        app.vim = Vim::new(vim::Mode::Insert);
        app.handle_leader_key_press();
        assert!(!app.leader_active);
    }

    // --- query_is_select ---

    #[test]
    fn query_is_select_detects_select() {
        assert!(query_is_select("SELECT 1"));
        assert!(query_is_select("  select * from foo"));
        assert!(query_is_select("WITH cte AS (SELECT 1) SELECT * FROM cte"));
    }

    #[test]
    fn query_is_select_detects_non_select() {
        assert!(!query_is_select("CREATE TABLE foo (a int)"));
        assert!(!query_is_select("INSERT INTO foo VALUES (1)"));
        assert!(!query_is_select("UPDATE foo SET a = 1"));
        assert!(!query_is_select("DELETE FROM foo"));
        assert!(!query_is_select("DROP TABLE foo"));
        assert!(!query_is_select("ALTER TABLE foo ADD COLUMN b int"));
    }

    #[test]
    fn query_is_select_values_and_table() {
        assert!(query_is_select("VALUES (1, 2), (3, 4)"));
        assert!(query_is_select("TABLE foo"));
    }

    // --- strip_trailing_semicolons ---

    #[test]
    fn strip_semicolon_removes_single() {
        assert_eq!(strip_trailing_semicolons("SELECT 1;"), "SELECT 1");
    }

    #[test]
    fn strip_semicolon_removes_multiple() {
        assert_eq!(strip_trailing_semicolons("SELECT 1;;;"), "SELECT 1");
    }

    #[test]
    fn strip_semicolon_handles_whitespace() {
        assert_eq!(strip_trailing_semicolons("SELECT 1 ;  "), "SELECT 1");
        assert_eq!(strip_trailing_semicolons("SELECT 1 ; ; "), "SELECT 1");
    }

    #[test]
    fn strip_semicolon_no_op_without_semicolon() {
        assert_eq!(strip_trailing_semicolons("SELECT 1"), "SELECT 1");
        assert_eq!(strip_trailing_semicolons("  SELECT 1  "), "  SELECT 1");
    }

    #[test]
    fn strip_semicolon_preserves_internal_semicolons() {
        // Semicolons inside the query (e.g. strings) are not touched
        assert_eq!(
            strip_trailing_semicolons("SELECT ';' AS s;"),
            "SELECT ';' AS s"
        );
    }

    // --- Pane visibility toggles ---

    #[test]
    fn toggle_sidebar_hides_and_shows() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(app.show_sidebar);
        app.toggle_pane_visibility(Focus::Sidebar);
        assert!(!app.show_sidebar);
        app.toggle_pane_visibility(Focus::Sidebar);
        assert!(app.show_sidebar);
    }

    #[test]
    fn toggle_files_hides_and_shows() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(!app.show_files);
        app.toggle_pane_visibility(Focus::Files);
        assert!(app.show_files);
        app.toggle_pane_visibility(Focus::Files);
        assert!(!app.show_files);
    }

    #[test]
    fn toggle_recent_hides_and_shows() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        assert!(!app.show_recent);
        app.toggle_pane_visibility(Focus::Recent);
        assert!(app.show_recent);
        app.toggle_pane_visibility(Focus::Recent);
        assert!(!app.show_recent);
    }

    #[test]
    fn toggle_focused_pane_moves_focus_to_editor() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::Sidebar;
        app.toggle_pane_visibility(Focus::Sidebar);
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn toggle_on_moves_focus_to_pane() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.show_files = false;
        app.focus = Focus::QueryEditor;
        app.toggle_pane_visibility(Focus::Files);
        assert_eq!(app.focus, Focus::Files);
    }

    #[test]
    fn toggle_noop_for_query_editor() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.focus = Focus::QueryEditor;
        // QueryEditor is not toggleable
        app.toggle_pane_visibility(Focus::QueryEditor);
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    // --- Focus cycling with new panes ---

    #[test]
    fn focus_cycles_through_all_visible_panes() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.show_sidebar = true;
        app.show_files = true;
        app.show_recent = true;
        app.results_visible = true;
        app.focus = Focus::Sidebar;

        // Sidebar -> Files -> QueryEditor -> Results -> Recent -> Sidebar
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Files);
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::QueryEditor);
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Results);
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Recent);
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn focus_skips_hidden_panes() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.show_sidebar = false;
        app.show_files = false;
        app.show_recent = false;
        app.results_visible = false;
        app.focus = Focus::QueryEditor;

        // Only QueryEditor visible — cycles to itself
        app.focus = app.next_visible_focus(true);
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn focus_reverse_with_all_panes() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.show_sidebar = true;
        app.show_files = true;
        app.show_recent = true;
        app.results_visible = true;
        app.focus = Focus::Sidebar;

        // Backwards: Sidebar -> Recent -> Results -> QueryEditor -> Files -> Sidebar
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::Recent);
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::Results);
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::QueryEditor);
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::Files);
        app.focus = app.next_visible_focus(false);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn visible_panes_default() {
        let app = App::new(AppConfig::default(), test_profiles(), None);
        // Default: sidebar + query editor
        let panes = app.visible_panes();
        assert_eq!(panes, vec![Focus::Sidebar, Focus::QueryEditor]);
    }

    #[test]
    fn visible_panes_all() {
        let mut app = App::new(AppConfig::default(), test_profiles(), None);
        app.show_sidebar = true;
        app.show_files = true;
        app.show_recent = true;
        app.results_visible = true;
        let panes = app.visible_panes();
        assert_eq!(panes, vec![
            Focus::Sidebar,
            Focus::Files,
            Focus::QueryEditor,
            Focus::Results,
            Focus::Recent,
        ]);
    }

    // --- Recents pane tests ---

    fn app_with_recents(count: usize) -> App<'static> {
        use crate::recents::RecentEntry;
        let mut app = test_app();
        app.focus = Focus::Recent;
        app.show_recent = true;
        app.recents = Recents::default();
        for i in 0..count {
            app.recents.entries.push(RecentEntry {
                query: format!("SELECT {i}"),
                connection: Some("testdb".into()),
                timestamp: 1700000000 + i as u64,
                duration_ms: 10,
                result: Some(db::QueryResult {
                    columns: vec!["x".into()],
                    rows: vec![vec![db::Value::Int(i as i64)]],
                }),
                error: None,
            });
        }
        app
    }

    #[test]
    fn recent_j_moves_selection_down() {
        let mut app = app_with_recents(3);
        assert_eq!(app.recents_selected, 0);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 1);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 2);
    }

    #[test]
    fn recent_j_clamps_at_end() {
        let mut app = app_with_recents(2);
        app.recents_selected = 1;
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 1);
    }

    #[test]
    fn recent_k_moves_selection_up() {
        let mut app = app_with_recents(3);
        app.recents_selected = 2;
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 1);
    }

    #[test]
    fn recent_k_clamps_at_zero() {
        let mut app = app_with_recents(3);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 0);
    }

    #[test]
    fn recent_enter_loads_query_into_editor() {
        let mut app = app_with_recents(2);
        app.recents_selected = 1;
        app.handle_recent_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let text: String = app.editor.lines().join("\n");
        assert_eq!(text, "SELECT 1");
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn recent_enter_resets_language_to_sql() {
        let mut app = app_with_recents(1);
        // Switch to PRQL first
        app.focus = Focus::QueryEditor;
        app.handle_leader_action(&key(KeyCode::Char('l')));
        assert_eq!(app.query_language, QueryLanguage::Prql);
        assert!(app.show_sql_preview);
        // Load a recent (always SQL) query
        app.focus = Focus::Recent;
        app.handle_recent_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.query_language, QueryLanguage::Sql);
        assert!(!app.show_sql_preview);
    }

    #[test]
    fn recent_leader_d_deletes_entry() {
        let mut app = app_with_recents(3);
        app.recents_selected = 1;
        app.handle_leader_key_press();
        app.handle_leader_action(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.recents.entries.len(), 2);
        assert_eq!(app.recents.entries[0].query, "SELECT 0");
        assert_eq!(app.recents.entries[1].query, "SELECT 2");
    }

    #[test]
    fn recent_leader_d_clamps_selection_after_last() {
        let mut app = app_with_recents(2);
        app.recents_selected = 1;
        app.handle_leader_key_press();
        app.handle_leader_action(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 0);
        assert_eq!(app.recents.entries.len(), 1);
    }

    #[test]
    fn recent_selection_populates_results() {
        let mut app = app_with_recents(3);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(app.results_visible);
        let result = app.query_result.as_ref().unwrap();
        assert_eq!(result.rows[0][0], db::Value::Int(1));
    }

    #[test]
    fn recent_navigation_empty_list_is_noop() {
        let mut app = app_with_recents(0);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.recents_selected, 0);
        app.handle_recent_key(&KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.recents.entries.len(), 0);
    }
}

#[cfg(test)]
mod files_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs;
    use tempfile::tempdir;

    fn app_with_files(root: std::path::PathBuf) -> App<'static> {
        App::new(AppConfig::default(), Profiles::default(), Some(root))
    }

    #[test]
    fn new_with_path_shows_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.sql"), "SELECT 1").unwrap();
        let app = app_with_files(dir.path().to_path_buf());
        assert!(app.show_files);
        assert!(!app.file_tree.is_empty());
        assert_eq!(app.file_tree_state.selected(), Some(0));
    }

    #[test]
    fn new_without_path_hides_files() {
        let app = App::new(AppConfig::default(), Profiles::default(), None);
        assert!(!app.show_files);
        assert!(app.file_tree.is_empty());
    }

    #[test]
    fn files_navigate_down_up() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.sql"), "").unwrap();
        fs::write(dir.path().join("b.sql"), "").unwrap();
        fs::write(dir.path().join("c.sql"), "").unwrap();
        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        assert_eq!(app.file_tree_state.selected(), Some(0));
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.file_tree_state.selected(), Some(1));
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.file_tree_state.selected(), Some(2));
        // Clamp at end
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.file_tree_state.selected(), Some(2));
        // Navigate up
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.file_tree_state.selected(), Some(1));
        // Clamp at start
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.file_tree_state.selected(), Some(0));
    }

    #[test]
    fn files_enter_expands_directory() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("inner.sql"), "SELECT 1").unwrap();

        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        // subdir is at index 0 (dirs first), collapsed
        assert!(!app.file_tree[0].expanded);
        app.handle_files_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Now expanded with real children
        assert!(app.file_tree[0].expanded);
        assert_eq!(app.file_tree[0].children[0].label, "inner.sql");
    }

    #[test]
    fn files_enter_opens_sql_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("query.sql"), "SELECT * FROM users").unwrap();

        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        app.handle_files_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let editor_text: String = app.editor.lines().join("\n");
        assert_eq!(editor_text, "SELECT * FROM users");
        assert_eq!(app.focus, Focus::QueryEditor);
    }

    #[test]
    fn files_enter_rejects_non_text_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("data.bin"), &[0u8, 1, 2, 3]).unwrap();

        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        app.handle_files_key(&KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.message.is_some());
        assert!(app.message.as_ref().unwrap().text.contains("Not a text file"));
        assert_eq!(app.focus, Focus::Files);
    }

    #[test]
    fn files_h_collapses_l_expands() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("file.sql"), "").unwrap();

        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        // Expand with 'l'
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert!(app.file_tree[0].expanded);

        // Collapse with 'h'
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert!(!app.file_tree[0].expanded);
    }

    #[test]
    fn files_q_does_not_quit() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.sql"), "").unwrap();
        let mut app = app_with_files(dir.path().to_path_buf());
        app.focus = Focus::Files;

        assert!(app.running);
        app.handle_files_key(&KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.running);
    }

    #[test]
    fn files_focus_cycles_when_visible() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.sql"), "").unwrap();
        let app = app_with_files(dir.path().to_path_buf());
        let panes = app.visible_panes();
        assert!(panes.contains(&Focus::Files));
    }
}

