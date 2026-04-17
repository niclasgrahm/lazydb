use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::ListState,
};
use tui_textarea::{Input, TextArea};

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::config::{AppConfig, Connection, Profiles};
use crate::db::{self, Database, SchemaNode};
use crate::db::clickhouse_backend::ClickHouse;
use crate::db::duckdb_backend::DuckDb;
use crate::db::postgres_backend::Postgres;
use crate::keybindings::Keybindings;
use crate::tree::TreeNode;
use crate::vim::{self, Transition, Vim};

#[derive(PartialEq)]
pub enum Focus {
    Sidebar,
    QueryEditor,
    Results,
}

#[derive(PartialEq)]
pub enum MessageLevel {
    Info,
    Error,
}

pub struct Message {
    pub text: String,
    pub level: MessageLevel,
}

pub const RESULTS_PAGE_SIZE: usize = 100;

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
}

impl<'a> App<'a> {
    pub fn new(config: AppConfig, profiles: Profiles) -> Self {
        let (sidebar_items, label_to_profile) = build_sidebar_tree(&profiles);

        let mut state = ListState::default();
        if !sidebar_items.is_empty() {
            state.select(Some(0));
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

    pub fn format_query(&mut self) {
        let query: String = self.editor.lines().join("\n");
        if query.trim().is_empty() {
            return;
        }
        let formatted = sqlformat::format(
            &query,
            &sqlformat::QueryParams::None,
            &sqlformat::FormatOptions {
                indent: sqlformat::Indent::Spaces(2),
                uppercase: Some(true),
                lines_between_queries: 1,
                ..Default::default()
            },
        );
        self.editor.select_all();
        self.editor.cut();
        self.editor.insert_str(&formatted);
    }

    pub fn execute_query(&mut self) {
        let query: String = self.editor.lines().join("\n");
        if query.trim().is_empty() {
            return;
        }
        self.results_query = Some(query.trim().to_string());
        self.results_page = 0;
        self.results_scroll_row = 0;
        self.results_scroll_col = 0;
        self.run_paged_query();
        self.refresh_schema();
    }

    fn run_paged_query(&mut self) {
        let Some(query) = &self.results_query else { return };
        let Some(conn) = self.connection.as_mut() else {
            self.show_error("No database connected");
            return;
        };
        let offset = self.results_page * RESULTS_PAGE_SIZE;
        let paged = format!(
            "SELECT * FROM ({query}) AS _lazydb_q LIMIT {limit} OFFSET {offset}",
            limit = RESULTS_PAGE_SIZE + 1,
        );
        let start = Instant::now();
        match conn.execute_query(&paged) {
            Ok(mut result) => {
                self.query_duration = Some(start.elapsed());
                self.results_has_more = result.rows.len() > RESULTS_PAGE_SIZE;
                if self.results_has_more {
                    result.rows.truncate(RESULTS_PAGE_SIZE);
                }
                self.query_result = Some(result);
                self.results_visible = true;
                self.focus = Focus::Results;
            }
            Err(e) => self.show_error(format!("Query error: {e}")),
        }
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
        let event = event::read()?;
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Press {
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

            let in_normal = self.focus != Focus::QueryEditor
                || self.vim.mode == vim::Mode::Normal;

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
                    self.focus = match self.focus {
                        Focus::Sidebar => Focus::QueryEditor,
                        Focus::QueryEditor if self.results_visible => Focus::Results,
                        Focus::QueryEditor => Focus::Sidebar,
                        Focus::Results => Focus::Sidebar,
                    };
                    return Ok(());
                }
                if self.keys.global.prev_pane.matches(key) {
                    self.focus = match self.focus {
                        Focus::Sidebar if self.results_visible => Focus::Results,
                        Focus::Sidebar => Focus::QueryEditor,
                        Focus::QueryEditor => Focus::Sidebar,
                        Focus::Results => Focus::QueryEditor,
                    };
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
                Focus::Sidebar => self.handle_sidebar_key(key),
                Focus::Results => self.handle_results_key(key),
            }
        }
        Ok(())
    }

    fn handle_sidebar_key(&mut self, key: &crossterm::event::KeyEvent) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let item_count = flat.len();
        let kb = &self.keys.sidebar;

        if kb.quit.matches(key) {
            self.running = false;
        } else if kb.navigate_up.matches(key) {
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
                let is_connection = flat.get(selected).is_some_and(|n| n.depth == 0);
                if is_connection {
                    self.toggle_connection(selected);
                } else {
                    TreeNode::toggle_at_index(&mut self.sidebar_items, selected);
                }
            }
        } else if kb.expand.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                TreeNode::toggle_at_index(&mut self.sidebar_items, selected);
            }
        } else if kb.collapse.matches(key) {
            if let Some(selected) = self.sidebar_state.selected() {
                TreeNode::collapse_at_index(&mut self.sidebar_items, selected);
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

        if kb.quit.matches(key) {
            self.running = false;
        } else if kb.close.matches(key) {
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

    fn toggle_connection(&mut self, flat_index: usize) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let Some(node) = flat.get(flat_index) else { return };
        let label = node.label.clone();

        if self.connected_db.as_ref() == Some(&label) {
            // Disconnect: drop connection, clear schema, collapse
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

            // Look up profile and connect
            let profile_key = self.label_to_profile.get(&label).cloned();
            let profile = profile_key
                .as_ref()
                .and_then(|k| self.profiles.connections.get(k));

            if let Some(profile) = profile {
                let result: Result<Box<dyn Database>, String> = match profile {
                    Connection::DuckDb(cfg) => {
                        DuckDb::connect(&cfg.path).map(|db| Box::new(db) as Box<dyn Database>)
                    }
                    Connection::Postgres(cfg) => {
                        Postgres::connect(&cfg.connection_string(), cfg.schema_name())
                            .map(|db| Box::new(db) as Box<dyn Database>)
                    }
                    Connection::ClickHouse(cfg) => {
                        ClickHouse::connect(
                            &cfg.url,
                            &cfg.database,
                            &cfg.user,
                            cfg.password.as_deref(),
                        )
                        .map(|db| Box::new(db) as Box<dyn Database>)
                    }
                };

                match result {
                    Ok(mut db) => {
                        match db.schema_tree() {
                            Ok(tree) => self.populate_schema(&label, tree),
                            Err(e) => self.show_error(format!("Schema error: {e}")),
                        }
                        self.connection = Some(db);
                        self.connected_db = Some(label.clone());
                    }
                    Err(e) => {
                        self.show_error(format!("Connection failed: {e}"));
                        return;
                    }
                }
            }

            // Expand the connection node
            let new_flat = TreeNode::flatten_all(&self.sidebar_items);
            if let Some(new_idx) = new_flat.iter().position(|n| n.label == label) {
                TreeNode::toggle_at_index(&mut self.sidebar_items, new_idx);
                self.sidebar_state.select(Some(new_idx));
            }
        }
    }

    fn preview_table(&mut self, flat_index: usize) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let Some(node) = flat.get(flat_index) else { return };

        // Find the nearest ancestor whose label is "Tables" or "Views"
        let parent_label = flat[..flat_index]
            .iter()
            .rev()
            .find(|n| n.depth < node.depth)
            .map(|n| n.label.as_str());

        if !matches!(parent_label, Some("Tables" | "Views")) {
            return;
        }

        let table_name = &node.label;
        let query = format!("SELECT * FROM {table_name} LIMIT 10");

        // Clear editor and insert the query
        self.editor.select_all();
        self.editor.cut();
        self.editor.insert_str(&query);
        self.focus = Focus::QueryEditor;
        self.vim = Vim::new(vim::Mode::Normal);
    }

    fn refresh_schema(&mut self) {
        let Some(label) = self.connected_db.clone() else { return };
        let Some(conn) = self.connection.as_mut() else { return };
        match conn.schema_tree() {
            Ok(tree) => self.populate_schema(&label, tree),
            Err(_) => {} // silent — don't interrupt the user's query result
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

