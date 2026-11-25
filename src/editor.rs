use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rhai::Dynamic;
use std::path::PathBuf;
use std::time::Duration;

use crate::buffer::Buffer;
use crate::clipboard::Clipboard;
use crate::commands::{self, Command};
use crate::config::Config;
use crate::plugins::Plugins;
use crate::syntax::Highlighter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus { Editor, Prompt }

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
#[allow(dead_code)] // variants are part of the plugin/prompt API surface
pub enum PromptAction {
    Find,
    Replace,
    SaveAs,
    Open,
    /// Enable or disable a plugin by name.
    Plugin { name: String, enable: bool },
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
    keytree: crate::keytree::KeyTree,
    /// Mouse bindings, mirroring micro's `RegisterMouseBinding`. Each entry maps
    /// an action name to a command; the tide/scroll behaviour is handled inline.
    mouse: Vec<(String, Command)>,
    search_opts: crate::search::SearchOptions,
}

/// The character a key event should insert literally, if any.
///
/// Terminals report a Shift-produced symbol as the finished character plus a
/// SHIFT modifier (`Shift-1` arrives as `Char('!')` with SHIFT set), so SHIFT
/// must not disqualify insertion — the character already carries its effect.
/// Only CONTROL and ALT mark a real chord that should never be typed.
fn literal_char(key: &KeyEvent) -> Option<char> {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    match key.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

/// Focus decides which mode is active, which in turn gates which bindings are
/// live — the same mechanism as micro's ModeConstraint.
fn sync_modes(keytree: &mut crate::keytree::KeyTree, prompt: bool, menu: bool) {
    keytree.set_mode("prompt", prompt);
    keytree.set_mode("menu", menu);
    keytree.set_mode("buffer", !prompt && !menu);
}

impl Editor {
    pub fn new(files: Vec<PathBuf>) -> Self {
        let config = Config::load_or_default();

        // Start from the built-in bindings, then apply user rebindings from
        // config.json (`keymap`: {"Ctrl-x Ctrl-s": "Save", ...}). A key may be a
        // multi-key chord like official micro's bindings.json.
        let mut keytree = commands::default_keytree();
        let mut bad_keys = 0usize;
        for (key_desc, cmd) in commands::default_sequences() {
            match commands::parse_sequence(key_desc) {
                Some(seq) => keytree.register(&seq, cmd, vec![]),
                None => bad_keys += 1,
            }
        }
        for (key_desc, cmd_name) in &config.keymap {
            match (commands::parse_sequence(key_desc), commands::parse_command(cmd_name)) {
                (Some(seq), Some(cmd)) => {
                    keytree.register(&seq, cmd, vec![crate::keytree::ModeConstraint::active_in("buffer")]);
                }
                _ => bad_keys += 1,
            }
        }
        // Mouse bindings from config override the built-in defaults.
        let mut mouse = Self::default_mouse();
        for (action, cmd_name) in &config.mouse {
            match commands::parse_command(cmd_name) {
                Some(cmd) => {
                    if let Some(slot) = mouse.iter_mut().find(|(a, _)| a == action) {
                        slot.1 = cmd;
                    } else {
                        mouse.push((action.clone(), cmd));
                    }
                }
                None => bad_keys += 1,
            }
        }
        // Load plugin scripts from ~/.config/micro-rs/plugins/*.rhai, skipping
        // the ones the user disabled in `config.plugins`.
        let mut plugins = Plugins::new();
        let plugin_dir = crate::config::Config::path()
            .parent().map(|p| p.join("plugins")).unwrap_or_default();
        if let Ok(entries) = std::fs::read_dir(&plugin_dir) {
            let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rhai"))
                .collect();
            paths.sort();
            for p in paths {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                if !config.plugin_enabled(&stem) {
                    continue;
                }
                let _ = plugins.load_script(&p);
            }
        }

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
            plugins,
            focus: Focus::Editor,
            prompt: None,
            status_message: if bad_keys > 0 {
                format!("Ready ({} bad keymap entries ignored)", bad_keys)
            } else {
                "Ready".into()
            },
            menu_open: None,
            menus,
            quit: false,
            keytree,
            mouse,
            search_opts: crate::search::SearchOptions::default(),
        }
    }

    /// Fire a plugin hook with a structured event map, plus the universal
    /// `onAnyEvent` hook used by official micro plugins.
    ///
    /// The map always carries `event`, `path` and `line`/`col` so a plugin can
    /// branch on context instead of only a bare string.
    fn fire_hook(&mut self, name: &str, arg: &str) {
        let map = self.event_map(name, arg);
        let _ = self.plugins.hook_map(name, map.clone());
        let _ = self.plugins.hook_map("onAnyEvent", map);
    }

    /// Fire an intercepting hook. Returns true when a plugin wants to block
    /// the action (official micro's `onBeforeTextEvent` semantics).
    fn fire_hook_bool(&mut self, name: &str, arg: &str) -> bool {
        let map = self.event_map(name, arg);
        let blocked = self.plugins.hook_map_bool(name, map.clone());
        let _ = self.plugins.hook_map("onAnyEvent", map);
        blocked
    }

    /// Build the context map handed to plugins.
    fn event_map(&self, name: &str, arg: &str) -> rhai::Map {
        let mut m = rhai::Map::new();
        m.insert("event".into(), Dynamic::from(name.to_string()));
        m.insert("arg".into(), Dynamic::from(arg.to_string()));
        let buf = &self.pane().buf;
        m.insert("line".into(), Dynamic::from(buf.cursor.line as i64));
        m.insert("col".into(), Dynamic::from(buf.cursor.col as i64));
        m.insert("dirty".into(), Dynamic::from(buf.dirty));
        m.insert("tab".into(), Dynamic::from(self.active_tab as i64));
        let path = self.tabs[self.active_tab]
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        m.insert("path".into(), Dynamic::from(path));
        m
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

    /// Default mouse bindings. Official micro names these actions
    /// `MouseLeft`/`MouseRight`/`MouseWheelUp`/`MouseWheelDown`/`MouseMiddle`.
    fn default_mouse() -> Vec<(String, Command)> {
        vec![
            ("MouseWheelUp".to_string(), Command::PageUp),
            ("MouseWheelDown".to_string(), Command::PageDown),
            ("MouseLeft".to_string(), Command::None),
            ("MouseRight".to_string(), Command::None),
        ]
    }

    fn mouse_command(&self, action: &str) -> Option<Command> {
        self.mouse.iter().find(|(a, _)| a == action).map(|(_, c)| c.clone())
    }

    /// Bind a plugin/config-supplied mouse action at runtime. Kept public so
    /// plugins and user config can rebind without rebuilding the editor.
    #[allow(dead_code)]
    pub fn register_mouse(&mut self, action: &str, cmd: Command) {
        if let Some(slot) = self.mouse.iter_mut().find(|(a, _)| a == action) {
            slot.1 = cmd;
        } else {
            self.mouse.push((action.to_string(), cmd));
        }
    }

    fn on_mouse(&mut self, ev: event::MouseEvent) {
        use crossterm::event::MouseEventKind;
        let action = match ev.kind {
            MouseEventKind::ScrollUp => "MouseWheelUp",
            MouseEventKind::ScrollDown => "MouseWheelDown",
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => "MouseLeft",
            MouseEventKind::Down(crossterm::event::MouseButton::Right) => "MouseRight",
            MouseEventKind::Down(crossterm::event::MouseButton::Middle) => "MouseMiddle",
            _ => return,
        };
        self.fire_hook("onMouse", action);
        if let Some(cmd) = self.mouse_command(action) {
            if cmd != Command::None {
                self.execute(cmd);
            }
        }
    }

    pub fn run_step(&mut self) -> Result<()> {
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        self.on_key(key);
                    }
                }
                Event::Mouse(me) => self.on_mouse(me),
                _ => {}
            }
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Fire the universal key hook before dispatching, mirroring
        // official micro's RunPluginFn("onAnyEvent", ...).
        self.fire_hook("onKey", &format!("{:?}", key.code));

        // Focus decides the active mode, which in turn gates which bindings are
        // live — the same mechanism as micro's ModeConstraint.
        sync_modes(&mut self.keytree, self.prompt.is_some(), self.menu_open.is_some());

        if let Some(prompt) = &mut self.prompt {
            match key.code {
                KeyCode::Esc => { self.prompt = None; self.focus = Focus::Editor; }
                KeyCode::Enter => { let p = self.prompt.take().unwrap(); self.focus = Focus::Editor; self.run_prompt(p); }
                KeyCode::Backspace => { prompt.input.pop(); }
                _ => {
                    // Same rule as the buffer: SHIFT-produced symbols and
                    // capitals are normal input, CONTROL/ALT are not.
                    if let Some(c) = literal_char(&key) {
                        prompt.input.push(c);
                    }
                }
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

        match self.keytree.next_event(key.code, key.modifiers) {
            crate::keytree::KeyOutcome::Command(cmd) => self.execute(cmd),
            crate::keytree::KeyOutcome::Pending => {
                self.status_message = format!("{} …", self.keytree.pending_label());
            }
            crate::keytree::KeyOutcome::None => {
                // Unbound plain character falls through to literal insertion,
                // like micro's DoRuneInsert fallback.
                if let Some(c) = literal_char(&key) {
                    self.execute(Command::Insert(c));
                }
            }
        }
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
            12 => { self.status_message = "micro-rs 0.1.3 — Rust terminal editor".into(); }
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
                    self.fire_hook("onBufferOpen", &path.display().to_string());
                    let tab = Self::new_tab(Some(path.clone()));
                    self.tabs[self.active_tab] = tab;
                    self.highlighter = Some(Highlighter::for_file(&path));
                    self.status_message = format!("Opened {}", path.display());
                } else {
                    self.status_message = "File not found".into();
                }
            }
            PromptAction::Plugin { name, enable } => {
                self.config.plugins.insert(name.clone(), enable);
                let _ = self.config.save();
                self.reload_plugins();
                self.status_message = format!(
                    "Plugin {} {} ({} loaded)",
                    name,
                    if enable { "enabled" } else { "disabled" },
                    self.plugins.count()
                );
            }
        }
    }

    /// Directory scanned for `.rhai` plugin scripts.
    fn plugin_dir() -> PathBuf {
        crate::config::Config::path()
            .parent()
            .map(|p| p.join("plugins"))
            .unwrap_or_default()
    }

    /// Every plugin script on disk, whether or not it is enabled.
    fn installed_plugins() -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(Self::plugin_dir()) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("rhai") {
                    continue;
                }
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                out.push((stem, p));
            }
        }
        out.sort();
        out
    }

    /// Drop every loaded script and re-read the plugin directory, applying the
    /// enable flags from config. Mirrors micro's plugin reload on toggle.
    fn reload_plugins(&mut self) {
        let mut fresh = Plugins::new();
        for (stem, path) in Self::installed_plugins() {
            if !self.config.plugin_enabled(&stem) {
                continue;
            }
            let _ = fresh.load_script(&path);
        }
        self.plugins = fresh;
    }

    /// `/plugin` style listing: name, enabled state, and its absolute path.
    fn list_plugins(&mut self) {
        let items = Self::installed_plugins();
        if items.is_empty() {
            self.status_message = format!("No plugins in {}", Self::plugin_dir().display());
            return;
        }
        let parts: Vec<String> = items
            .iter()
            .map(|(stem, _)| {
                let on = self.config.plugin_enabled(stem);
                format!("{stem}{}", if on { "" } else { " [off]" })
            })
            .collect();
        self.status_message = format!("Plugins: {}", parts.join(", "));
    }

    /// Reload plugins from disk on demand (`^P` in the menu, or a key binding
    /// to `ReloadPlugins`).
    fn reload_plugins_cmd(&mut self) {
        self.reload_plugins();
        self.status_message = format!("Reloaded {} plugin(s)", self.plugins.count());
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
        self.fire_hook("onSave", &path.display().to_string());
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
        let next = (cur + 1) % names.len();
        self.fire_hook("onBufferOptionChanged", &format!("theme:{}", names[next]));
        self.config.theme = names[next].clone();
        let _ = self.config.save();
        self.status_message = format!("Theme: {}", self.config.theme);
    }

    fn execute(&mut self, cmd: Command) {
        // Intercept text-mutating commands with the official-style
        // `onBeforeTextEvent` hook: a plugin returning true cancels the edit.
        let text_cmd = match &cmd {
            Command::Insert(_) => Some("Insert"),
            Command::Backspace => Some("Backspace"),
            Command::Delete => Some("Delete"),
            Command::Newline => Some("Newline"),
            Command::Tab => Some("Tab"),
            Command::Paste => Some("Paste"),
            _ => None,
        };
        if let Some(t) = text_cmd {
            if self.fire_hook_bool("onBeforeTextEvent", t) {
                return;
            }
        }

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
            Command::NextTab => {
                self.active_tab = (self.active_tab + 1) % self.tabs.len();
                self.fire_hook("onSetActive", &self.active_tab.to_string());
            }
            Command::PrevTab => {
                self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
                self.fire_hook("onSetActive", &self.active_tab.to_string());
            }
            Command::ToggleTheme => self.cycle_theme(),
            Command::ReloadPlugins => self.reload_plugins_cmd(),
            Command::ListPlugins => self.list_plugins(),
            Command::ListKeybindings => {
                let all = self.keytree.bindings();
                self.status_message = if all.is_empty() {
                    "No key bindings".to_string()
                } else {
                    format!("{} binding(s): {}", all.len(), all.iter().take(12)
                        .map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("  "))
                };
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn shift_symbols_are_typed() {
        // Terminals report Shift-1 as the finished character plus SHIFT.
        assert_eq!(literal_char(&key(KeyCode::Char('!'), KeyModifiers::SHIFT)), Some('!'));
        assert_eq!(literal_char(&key(KeyCode::Char('@'), KeyModifiers::SHIFT)), Some('@'));
        // A capital letter arrives the same way.
        assert_eq!(literal_char(&key(KeyCode::Char('A'), KeyModifiers::SHIFT)), Some('A'));
        assert_eq!(literal_char(&key(KeyCode::Char('a'), KeyModifiers::NONE)), Some('a'));
    }

    #[test]
    fn chords_are_not_typed() {
        assert_eq!(literal_char(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)), None);
        assert_eq!(literal_char(&key(KeyCode::Char('p'), KeyModifiers::ALT)), None);
        assert_eq!(literal_char(&key(KeyCode::Enter, KeyModifiers::NONE)), None);
    }
}
