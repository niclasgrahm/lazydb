use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    style::{Color, Style},
    widgets::ListState,
};
use tui_textarea::{Input, TextArea};

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
}

impl<'a> App<'a> {
    pub fn new() -> Self {
        let sidebar_items = demo_tree();

        let mut state = ListState::default();
        state.select(Some(0));

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
        let item_count = TreeNode::flatten_all(&self.sidebar_items).len();
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
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
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
}

fn demo_tree() -> Vec<TreeNode> {
    vec![
        TreeNode::connection(
            "prod-postgres",
            vec![
                TreeNode::folder(
                    "Tables",
                    vec![
                        TreeNode::leaf("users"),
                        TreeNode::leaf("orders"),
                        TreeNode::leaf("products"),
                    ],
                ),
                TreeNode::folder(
                    "Views",
                    vec![
                        TreeNode::leaf("active_users"),
                        TreeNode::leaf("order_summary"),
                    ],
                ),
            ],
        ),
        TreeNode::connection(
            "dev-sqlite",
            vec![TreeNode::folder(
                "Tables",
                vec![
                    TreeNode::leaf("migrations"),
                    TreeNode::leaf("settings"),
                ],
            )],
        ),
    ]
}
