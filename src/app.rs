use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    style::{Color, Style},
    widgets::ListState,
};
use tui_textarea::{Input, TextArea};

use crate::config::{AppConfig, Profiles};
use crate::tree::TreeNode;
use crate::vim::{self, Transition, Vim};

#[derive(PartialEq)]
pub enum Focus {
    Sidebar,
    QueryEditor,
    Results,
}

pub struct App<'a> {
    pub sidebar_items: Vec<TreeNode>,
    pub sidebar_state: ListState,
    pub editor: TextArea<'a>,
    pub vim: Vim,
    pub results_visible: bool,
    pub results_content: String,
    pub focus: Focus,
    pub running: bool,
    pub sidebar_width: u16,
    pub connected_db: Option<String>,
}

impl<'a> App<'a> {
    pub fn new(config: AppConfig, profiles: Profiles) -> Self {
        let sidebar_items = build_sidebar_tree(&profiles);

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
            results_content: String::new(),
            focus: Focus::Sidebar,
            running: true,
            sidebar_width: config.sidebar_width,
            connected_db: None,
        }
    }

    pub fn execute_query(&mut self) {
        let query: String = self.editor.lines().join("\n");
        if !query.trim().is_empty() {
            self.results_content = format!(
                "Executed: {}\n\n(no database connected — results will appear here)",
                query.trim()
            );
            self.results_visible = true;
            self.focus = Focus::Results;
        }
    }

    pub fn handle_event(&mut self) -> Result<()> {
        let event = event::read()?;
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }

            if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.execute_query();
                return Ok(());
            }

            let in_normal = self.vim.mode == vim::Mode::Normal;
            if key.code == KeyCode::Tab && (self.focus != Focus::QueryEditor || in_normal) {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::QueryEditor,
                    Focus::QueryEditor if self.results_visible => Focus::Results,
                    Focus::QueryEditor => Focus::Sidebar,
                    Focus::Results => Focus::Sidebar,
                };
                return Ok(());
            }
            if key.code == KeyCode::BackTab {
                self.focus = match self.focus {
                    Focus::Sidebar if self.results_visible => Focus::Results,
                    Focus::Sidebar => Focus::QueryEditor,
                    Focus::QueryEditor => Focus::Sidebar,
                    Focus::Results => Focus::QueryEditor,
                };
                return Ok(());
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
                Focus::Sidebar => match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Esc => self.running = false,
                    _ => self.handle_sidebar_key(key.code),
                },
                Focus::Results => match key.code {
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Esc => {
                        self.results_visible = false;
                        self.focus = Focus::QueryEditor;
                    }
                    KeyCode::Char('c') => {
                        self.results_visible = false;
                        self.focus = Focus::QueryEditor;
                    }
                    _ => {}
                },
            }
        }
        Ok(())
    }

    fn handle_sidebar_key(&mut self, code: KeyCode) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let item_count = flat.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                let selected = self.sidebar_state.selected().unwrap_or(0);
                if selected > 0 {
                    self.sidebar_state.select(Some(selected - 1));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let selected = self.sidebar_state.selected().unwrap_or(0);
                if selected + 1 < item_count {
                    self.sidebar_state.select(Some(selected + 1));
                }
            }
            KeyCode::Enter => {
                if let Some(selected) = self.sidebar_state.selected() {
                    let is_connection = flat.get(selected).is_some_and(|n| n.depth == 0);
                    if is_connection {
                        self.toggle_connection(selected);
                    } else {
                        TreeNode::toggle_at_index(&mut self.sidebar_items, selected);
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(selected) = self.sidebar_state.selected() {
                    TreeNode::toggle_at_index(&mut self.sidebar_items, selected);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(selected) = self.sidebar_state.selected() {
                    TreeNode::collapse_at_index(&mut self.sidebar_items, selected);
                }
            }
            _ => {}
        }
    }

    fn toggle_connection(&mut self, flat_index: usize) {
        let flat = TreeNode::flatten_all(&self.sidebar_items);
        let Some(node) = flat.get(flat_index) else { return };
        let label = node.label.clone();

        if self.connected_db.as_ref() == Some(&label) {
            // Disconnect: collapse and clear
            self.connected_db = None;
            TreeNode::collapse_at_index(&mut self.sidebar_items, flat_index);
        } else {
            // Collapse previously connected db
            if let Some(prev) = &self.connected_db {
                for node in self.sidebar_items.iter_mut() {
                    if node.label == *prev && node.expanded {
                        node.expanded = false;
                        break;
                    }
                }
            }
            // Connect and expand the new one — recompute flat index after collapse
            self.connected_db = Some(label.clone());
            let new_flat = TreeNode::flatten_all(&self.sidebar_items);
            if let Some(new_idx) = new_flat.iter().position(|n| n.label == label) {
                TreeNode::toggle_at_index(&mut self.sidebar_items, new_idx);
                self.sidebar_state.select(Some(new_idx));
            }
        }
    }
}

fn build_sidebar_tree(profiles: &Profiles) -> Vec<TreeNode> {
    profiles
        .connections
        .iter()
        .map(|(name, conn)| {
            let label = format!("{} ({})", name, conn.type_name());
            TreeNode::connection(
                &label,
                vec![
                    TreeNode::folder("Tables", vec![]),
                    TreeNode::folder("Views", vec![]),
                ],
            )
        })
        .collect()
}
