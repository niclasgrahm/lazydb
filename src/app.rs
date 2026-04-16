use color_eyre::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{
    style::{Color, Style},
    widgets::ListState,
};
use tui_textarea::{Input, TextArea};

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::config::{AppConfig, Connection, Profiles};
use crate::db::{self, Database};
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

    pub fn execute_query(&mut self) {
        let query: String = self.editor.lines().join("\n");
        if query.trim().is_empty() {
            return;
        }
        let Some(conn) = self.connection.as_mut() else {
            self.show_error("No database connected");
            return;
        };
        let start = Instant::now();
        match conn.execute_query(query.trim()) {
            Ok(result) => {
                self.query_duration = Some(start.elapsed());
                self.query_result = Some(result);
                self.results_visible = true;
                self.focus = Focus::Results;
                self.refresh_schema();
            }
            Err(e) => self.show_error(format!("Query error: {e}")),
        }
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
        if kb.quit.matches(key) {
            self.running = false;
        } else if kb.close.matches(key) {
            self.results_visible = false;
            self.focus = Focus::QueryEditor;
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
                        match db.schema_info() {
                            Ok(schema) => self.populate_schema(&label, schema),
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

        // Must be a leaf at depth 2 (under Tables or Views)
        if node.depth != 2 || node.has_children {
            return;
        }

        // Walk backwards to find the parent folder name
        let parent_label = flat[..flat_index]
            .iter()
            .rev()
            .find(|n| n.depth == 1)
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
        match conn.schema_info() {
            Ok(schema) => self.populate_schema(&label, schema),
            Err(_) => {} // silent — don't interrupt the user's query result
        }
    }

    fn populate_schema(&mut self, connection_label: &str, schema: db::SchemaInfo) {
        for node in self.sidebar_items.iter_mut() {
            if node.label == connection_label {
                for child in node.children.iter_mut() {
                    match child.label.as_str() {
                        "Tables" => {
                            child.children =
                                schema.tables.iter().map(|t| TreeNode::leaf(t)).collect();
                        }
                        "Views" => {
                            child.children =
                                schema.views.iter().map(|v| TreeNode::leaf(v)).collect();
                        }
                        _ => {}
                    }
                }
                break;
            }
        }
    }

    fn clear_schema(&mut self, connection_label: &str) {
        for node in self.sidebar_items.iter_mut() {
            if node.label == connection_label {
                for child in node.children.iter_mut() {
                    child.children.clear();
                }
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
            TreeNode::connection(
                &label,
                vec![
                    TreeNode::folder("Tables", vec![]),
                    TreeNode::folder("Views", vec![]),
                ],
            )
        })
        .collect();
    (nodes, label_map)
}

