use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::path::PathBuf;
use std::time::Duration;

use crate::buffer::Buffer;
use crate::clipboard::Clipboard;
use crate::commands::{self, Command};
use crate::config::Config;
use crate::plugins::Plugins;
use crate::syntax::Highlighter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus { Editor, Prompt, Menu }

pub struct Pane {
    pub buf: Buffer,
    pub top: usize,
}

pub struct Tab {
    pub path: Option<PathBuf>,
    pub panes: Vec<Pane>,
    pub active_pane: usize,
    pub vertical: bool,
}

pub struct Prompt {
    pub label: String,
    pub input: String,
    pub action: PromptAction,
}

#[derive(Clone, Debug)]
pub enum PromptAction {
    Find,
    Replace,
    SaveAs,
    Open,
    Menu(usize),
}

pub struct Editor {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
    pub config: Config,
    pub highlighter: Option<Highlighter>,
    pub clipboard: Clipboard,
    pub plugins: Plugins,
    pub focus: Focus,
    pub prompt: Option<Prompt>,
    pub status_message: String,
    pub menu_open: Option<usize>,
    pub menus: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
    pub quit: bool,
    keymap: std::collections::HashMap<(KeyCode, KeyModifiers), Command>,
    search_opts: crate::search::SearchOptions,
}

impl Editor {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let config = Config::load_or_default();
        let mut tabs = Vec::new();
        if files.is_empty() {
            tabs.push(Self::new_tab(None));
        } else {
            for p in files {
                tabs.push(Self::new_tab(Some(p)));
            }
        }
        let highlighter = tabs[0].path.as_ref().map(|p| Highlighter::for_file(p));
        let menus = vec![
            ("File", vec![("^S", "Save"), ("^O", "Open"), ("^Q", "Quit")]),
            ("Edit", vec![("^Z", "Undo"), ("^Y", "Redo"), ("^F", "Find"), ("^R", "Replace")]),
            ("View", vec![("F2", "Theme"), ("^W", "Split")]),
            ("Help", vec![("F1", "About")]),
        ];
        Self {
            tabs,
            active_tab: 0,
            config,
            highlighter,
            clipboard: Clipboard::new(),
            plugins: Plugins::new(),
            focus: Focus::Editor,
            prompt: None,
            status_message: "Ready".into(),
            menu_open: None,
            menus,
            quit: false,
            keymap: commands::default_keymap(),
            search_opts: crate::search::SearchOptions::default(),
        }
    }

    fn new_tab(path: Option<PathBuf>) -> Tab {
        let text = path.as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        Tab {
            path,
            panes: vec![Pane { buf: Buffer::new(&text), top: 0 }],
            active_pane: 0,
            vertical: false,
        }
    }

    fn pane(&self) -> &Pane {
        let t = &self.tabs[self.active_tab];
        &t.panes[t.active_pane]
    }

    fn pane_mut(&mut self) -> &mut Pane {
        let t = &mut self.tabs[self.active_tab];
        &mut t.panes[t.active_pane]
    }

    pub fn run_step(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    self.on_key(key);
                }
            }
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) {
        if let Some(prompt) = &mut self.prompt {
            match key.code {
                KeyCode::Esc => { self.prompt = None; self.focus = Focus::Editor; }
                KeyCode::Enter => { let p = self.prompt.take().unwrap(); self.focus = Focus::Editor; self.run_prompt(p); }
                KeyCode::Backspace => { prompt.input.pop(); }
                KeyCode::Char(c) => { prompt.input.push(c); }
                _ => {}
            }
            return;
        }

        if let Some(idx) = self.menu_open {
            match key.code {
                KeyCode::Esc => { self.menu_open = None; self.focus = Focus::Editor; }
                KeyCode::Left => { self.menu_open = Some(idx.saturating_sub(1)); }
                KeyCode::Right => { self.menu_open = Some((idx + 1).min(self.menus.len() - 1)); }
                KeyCode::Enter => { self.run_menu_action(idx); }
                _ => {}
            }
            return;
        }

        let cmd = self.keymap.get(&(key.code, key.modifiers)).cloned().unwrap_or_else(|| {
            if key.modifiers.is_empty() {
                match key.code {
                    KeyCode::Char(c) => Command::Insert(c),
                    _ => Command::None,
                }
            } else { Command::None }
        });
        self.execute(cmd);
    }

    fn run_menu_action(&mut self, idx: usize) {
        self.menu_open = None;
        self.focus = Focus::Editor;
        match idx {
            0 => self.prompt_for_save_as(),
            1 => self.prompt_for_open(),
            2 => self.execute(Command::Quit),
            4 => self.execute(Command::Undo),
            5 => self.execute(Command::Redo),
            6 => self.execute(Command::Find),
            7 => self.execute(Command::Replace),
            9 => self.cycle_theme(),
            10 => self.execute(Command::SplitVertical),
            12 => { self.status_message = "micro-rs 0.1.0 — Rust terminal editor".into(); }
            _ => {}
        }
    }

    fn run_prompt(&mut self, p: Prompt) {
        match p.action {
            PromptAction::Find => {
                let needle = p.input;
                let text = self.pane().buf.to_string();
                let matches = crate::search::find_all(&text, &needle, &self.search_opts);
                if let Some(&(start, _)) = matches.first() {
                    let ch_to_cursor = |ch: usize| {
                        let rope = &self.pane().buf.rope;
                        let line = rope.char_to_line(ch.min(rope.len_chars()));
                        let ls = rope.line_to_char(line);
                        crate::buffer::Cursor { line, col: ch.saturating_sub(ls) }
                    };
                    self.pane_mut().buf.cursor = ch_to_cursor(start);
                    self.status_message = format!("{} match(es)", matches.len());
                } else {
                    self.status_message = "Not found".into();
                }
            }
            PromptAction::Replace => {
                self.status_message = format!("Replace: '{}' (use ^R again to implement)", p.input);
            }
            PromptAction::SaveAs => {
                let path = PathBuf::from(p.input);
                self.save_to(path);
            }
            PromptAction::Open => {
                let path = PathBuf::from(p.input);
                if path.exists() {
                    let tab = Self::new_tab(Some(path.clone()));
                    self.tabs[self.active_tab] = tab;
                    self.highlighter = Some(Highlighter::for_file(&path));
                    self.status_message = format!("Opened {}", path.display());
                } else {
                    self.status_message = "File not found".into();
                }
            }
            PromptAction::Menu(idx) => { self.run_menu_action(idx); }
        }
    }

    fn prompt_for_save_as(&mut self) {
        let cur = self.tabs[self.active_tab].path.as_ref()
            .map(|p| p.display().to_string()).unwrap_or_default();
        self.prompt = Some(Prompt { label: "Save as: ".into(), input: cur, action: PromptAction::SaveAs });
        self.focus = Focus::Prompt;
    }

    fn prompt_for_open(&mut self) {
        self.prompt = Some(Prompt { label: "Open: ".into(), input: String::new(), action: PromptAction::Open });
        self.focus = Focus::Prompt;
    }

    fn save_to(&mut self, path: PathBuf) {
        let text = self.pane().buf.to_string();
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.tabs[self.active_tab].path = Some(path.clone());
                self.pane_mut().buf.dirty = false;
                self.status_message = format!("Saved {}", path.display());
            }
            Err(e) => self.status_message = format!("Save failed: {e}"),
        }
    }

    fn save(&mut self) {
        if let Some(path) = self.tabs[self.active_tab].path.clone() {
            self.save_to(path);
        } else {
            self.prompt_for_save_as();
        }
    }

    fn cycle_theme(&mut self) {
        let names = self.highlighter.as_ref().map(|h| h.theme_names()).unwrap_or_default();
        if names.is_empty() { return; }
        let cur = names.iter().position(|n| *n == self.config.theme).unwrap_or(0);
        self.config.theme = names[(cur + 1) % names.len()].clone();
        self.status_message = format!("Theme: {}", self.config.theme);
    }

    fn execute(&mut self, cmd: Command) {
        match cmd {
            Command::Quit => self.quit = true,
            Command::Save => self.save(),
            Command::SaveAs => self.prompt_for_save_as(),
            Command::Open => self.prompt_for_open(),
            Command::Undo => self.pane_mut().buf.undo(),
            Command::Redo => self.pane_mut().buf.redo(),
            Command::Copy => {
                if let Some(s) = self.pane().buf.selected_text() {
                    self.clipboard.set(&s);
                    self.status_message = "Copied".into();
                }
            }
            Command::Cut => {
                if self.pane_mut().buf.delete_selection() {
                    let s = self.clipboard.get();
                    let _ = s;
                    self.status_message = "Cut".into();
                }
            }
            Command::Paste => {
                let s = self.clipboard.get();
                if !s.is_empty() { self.pane_mut().buf.insert_str(&s); }
            }
            Command::SelectAll => {
                self.pane_mut().buf.cursor = crate::buffer::Cursor { line: self.pane().buf.line_count().saturating_sub(1), col: self.pane().buf.line(self.pane().buf.line_count().saturating_sub(1)).len() };
                self.pane_mut().buf.sel_anchor = Some(crate::buffer::Cursor::default());
            }
            Command::Find => {
                self.prompt = Some(Prompt { label: "Find: ".into(), input: String::new(), action: PromptAction::Find });
                self.focus = Focus::Prompt;
            }
            Command::Replace => {
                self.prompt = Some(Prompt { label: "Replace: ".into(), input: String::new(), action: PromptAction::Replace });
                self.focus = Focus::Prompt;
            }
            Command::FindNext => self.status_message = "Find next".into(),
            Command::MoveLeft => self.pane_mut().buf.move_left(),
            Command::MoveRight => self.pane_mut().buf.move_right(),
            Command::MoveUp => self.pane_mut().buf.move_up(),
            Command::MoveDown => self.pane_mut().buf.move_down(),
            Command::Home => self.pane_mut().buf.move_home(),
            Command::End => self.pane_mut().buf.move_end(),
            Command::FileStart => self.pane_mut().buf.move_file_start(),
            Command::FileEnd => self.pane_mut().buf.move_file_end(),
            Command::PageUp => {
                for _ in 0..10 { self.pane_mut().buf.move_up(); }
            }
            Command::PageDown => {
                for _ in 0..10 { self.pane_mut().buf.move_down(); }
            }
            Command::Insert(c) => self.pane_mut().buf.insert_char(c),
            Command::Backspace => { self.pane_mut().buf.delete_selection(); self.pane_mut().buf.backspace(); }
            Command::Delete => { self.pane_mut().buf.delete_selection(); self.pane_mut().buf.delete_char(); }
            Command::Newline => self.pane_mut().buf.newline(),
            Command::Tab => {
                let n = self.config.tab_size;
                self.pane_mut().buf.insert_str(&" ".repeat(n));
            }
            Command::AddCursorAbove => {
                let c = self.pane().buf.cursor;
                if c.line > 0 {
                    self.pane_mut().buf.extra_cursors.push(crate::buffer::Cursor { line: c.line - 1, col: c.col });
                }
            }
            Command::AddCursorBelow => {
                let c = self.pane().buf.cursor;
                self.pane_mut().buf.extra_cursors.push(crate::buffer::Cursor { line: c.line + 1, col: c.col });
            }
            Command::SplitVertical => {
                let t = &mut self.tabs[self.active_tab];
                if t.panes.len() < 4 {
                    t.panes.push(Pane { buf: Buffer::new(""), top: 0 });
                    t.vertical = false;
                    t.active_pane = t.panes.len() - 1;
                    self.status_message = "Split pane".into();
                }
            }
            Command::SplitHorizontal => {
                let t = &mut self.tabs[self.active_tab];
                if t.panes.len() < 4 {
                    t.panes.push(Pane { buf: Buffer::new(""), top: 0 });
                    t.vertical = true;
                    t.active_pane = t.panes.len() - 1;
                    self.status_message = "Split pane".into();
                }
            }
            Command::NextPane => {
                let t = &mut self.tabs[self.active_tab];
                t.active_pane = (t.active_pane + 1) % t.panes.len();
            }
            Command::NextTab => self.active_tab = (self.active_tab + 1) % self.tabs.len(),
            Command::PrevTab => self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len(),
            Command::ToggleTheme => self.cycle_theme(),
            Command::Help => { self.status_message = "micro-rs: ^S save  ^F find  ^Z undo  ^Q quit".into(); }
            Command::Cancel => { self.prompt = None; self.menu_open = None; self.focus = Focus::Editor; }
            Command::Confirm => {}
            Command::None => {}
        }
        // keep cursor visible
        let c = self.pane().buf.cursor;
        let p = self.pane_mut();
        if c.line < p.top { p.top = c.line; }
    }
}
