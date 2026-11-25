use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Quit,
    Save,
    SaveAs,
    Open,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Find,
    Replace,
    FindNext,
    SelectAll,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Home,
    End,
    PageUp,
    PageDown,
    FileStart,
    FileEnd,
    Insert(char),
    Backspace,
    Delete,
    Newline,
    Tab,
    AddCursorAbove,
    AddCursorBelow,
    SplitVertical,
    SplitHorizontal,
    NextTab,
    PrevTab,
    NextPane,
    ToggleTheme,
    Help,
    Cancel,
    Confirm,
    /// Re-read plugin scripts from disk.
    ReloadPlugins,
    /// List installed plugins and their enable state.
    ListPlugins,
    /// Print every registered key binding to the status bar.
    ListKeybindings,
    /// Toggle folding of the region at the cursor.
    ToggleFold,
    /// Expand every folded region.
    UnfoldAll,
    None,
}

pub fn default_keymap() -> std::collections::HashMap<(KeyCode, KeyModifiers), Command> {    use KeyCode::*;
    let mut m = std::collections::HashMap::new();
    let c = KeyModifiers::CONTROL;
    let alt = KeyModifiers::ALT;
    let shift = KeyModifiers::SHIFT;
    m.insert((Char('q'), c), Command::Quit);
    m.insert((Char('s'), c), Command::Save);
    m.insert((Char('o'), c), Command::Open);
    m.insert((Char('z'), c), Command::Undo);
    m.insert((Char('y'), c), Command::Redo);
    m.insert((Char('x'), c), Command::Cut);
    m.insert((Char('c'), c), Command::Copy);
    m.insert((Char('v'), c), Command::Paste);
    m.insert((Char('f'), c), Command::Find);
    m.insert((Char('r'), c), Command::Replace);
    m.insert((Char('n'), c), Command::FindNext);
    m.insert((Char('a'), c), Command::SelectAll);
    m.insert((Char('p'), alt), Command::AddCursorAbove);
    m.insert((Char('n'), alt), Command::AddCursorBelow);
    m.insert((Left, c), Command::Home);
    m.insert((Right, c), Command::End);
    m.insert((Up, c), Command::FileStart);
    m.insert((Down, c), Command::FileEnd);
    m.insert((PageUp, c), Command::PageUp);
    m.insert((PageDown, c), Command::PageDown);
    m.insert((Left, shift), Command::MoveLeft);
    m.insert((Right, shift), Command::MoveRight);
    m.insert((Up, shift), Command::MoveUp);
    m.insert((Down, shift), Command::MoveDown);
    m.insert((Left, KeyModifiers::NONE), Command::MoveLeft);
    m.insert((Right, KeyModifiers::NONE), Command::MoveRight);
    m.insert((Up, KeyModifiers::NONE), Command::MoveUp);
    m.insert((Down, KeyModifiers::NONE), Command::MoveDown);
    m.insert((Home, KeyModifiers::NONE), Command::Home);
    m.insert((End, KeyModifiers::NONE), Command::End);
    m.insert((Backspace, KeyModifiers::NONE), Command::Backspace);
    m.insert((Delete, KeyModifiers::NONE), Command::Delete);
    m.insert((Enter, KeyModifiers::NONE), Command::Newline);
    m.insert((Tab, KeyModifiers::NONE), Command::Tab);
    m.insert((Esc, KeyModifiers::NONE), Command::Cancel);
    m.insert((F(2), KeyModifiers::NONE), Command::ToggleTheme);
    m.insert((F(3), KeyModifiers::NONE), Command::ToggleFold);
    m.insert((F(4), KeyModifiers::NONE), Command::UnfoldAll);
    m
}

/// Build the runtime key tree from the built-in defaults. Bindings are guarded
/// by the `buffer` mode so prompt/menu focus can disable them wholesale, exactly
/// like official micro's mode constraints.
pub fn default_keytree() -> crate::keytree::KeyTree {
    let mut tree = crate::keytree::KeyTree::new();
    for ((code, mods), cmd) in default_keymap() {
        tree.register(
            &[crate::keytree::KeyPattern::exact(code, mods)],
            cmd,
            vec![crate::keytree::ModeConstraint::active_in("buffer")],
        );
    }
    tree
}

/// A few multi-key sequences mirroring micro's common chords, registered as
/// their own bindings so `Ctrl-x Ctrl-s` style prefixes work out of the box.
pub fn default_sequences() -> Vec<(&'static str, Command)> {
    vec![
        ("Ctrl-x Ctrl-s", Command::Save),
        ("Ctrl-x Ctrl-c", Command::Quit),
        ("Ctrl-x Ctrl-f", Command::Open),
        ("Ctrl-x 2", Command::SplitHorizontal),
        ("Ctrl-x 3", Command::SplitVertical),
        ("Ctrl-x o", Command::NextPane),
        ("Alt-w any", Command::Cancel),
    ]
}

/// Parse a key description such as `Ctrl-s`, `Alt-p`, `Ctrl-Shift-Left`,
/// `F2`, `Enter` into the (KeyCode, KeyModifiers) pair used by the runtime
/// keymap. Returns None for unknown descriptions.
pub fn parse_key(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let mut mods = KeyModifiers::NONE;
    let mut parts: Vec<&str> = s.split('-').collect();
    for _ in 0..2 {
        if parts.is_empty() { break; }
        let prefix = parts[0].trim().to_ascii_lowercase();
        let matched = match prefix.as_str() {
            "ctrl" | "control" => { mods |= KeyModifiers::CONTROL; true }
            "alt" | "option" => { mods |= KeyModifiers::ALT; true }
            "shift" => { mods |= KeyModifiers::SHIFT; true }
            _ => false,
        };
        if matched { parts.remove(0); } else { break; }
    }
    let key = parts.join("-");
    let lc = key.trim().to_ascii_lowercase();
    let code = match lc.as_str() {
        "enter" | "return" => KeyCode::Enter,
        "backspace" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "tab" => KeyCode::Tab,
        "esc" | "escape" => KeyCode::Esc,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "space" => KeyCode::Char(' '),
        c if c.len() > 1 && c.starts_with('f') && c[1..].parse::<u8>().is_ok() => {
            KeyCode::F(c[1..].parse::<u8>().ok()?)
        }
        c if c.chars().count() == 1 => KeyCode::Char(c.chars().next()?),
        _ => return None,
    };
    Some((code, mods))
}

/// Parse a key *sequence* from config, e.g. `"Ctrl-x Ctrl-s"` or `"Alt-w any"`.
/// Space-separated keys form a multi-key sequence like official micro's
/// `bindings.json`. A single descriptor is also accepted.
pub fn parse_sequence(s: &str) -> Option<Vec<crate::keytree::KeyPattern>> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for part in trimmed.split_whitespace() {
        if part.eq_ignore_ascii_case("any") {
            out.push(crate::keytree::KeyPattern::any());
            continue;
        }
        let (code, mods) = parse_key(part)?;
        out.push(crate::keytree::KeyPattern::exact(code, mods));
    }
    Some(out)
}

/// Parse a command name from config. Names are case-insensitive and may use
/// `_`, `-` or spaces as separators. `Insert(ch)` is intentionally not
/// configurable through a name.
pub fn parse_command(s: &str) -> Option<Command> {
    let n: String = s.trim().to_ascii_lowercase().chars()
        .filter(|c| *c != '_' && *c != '-' && *c != ' ').collect();
    Some(match n.as_str() {
        "quit" => Command::Quit,
        "save" => Command::Save,
        "saveas" => Command::SaveAs,
        "open" => Command::Open,
        "undo" => Command::Undo,
        "redo" => Command::Redo,
        "cut" => Command::Cut,
        "copy" => Command::Copy,
        "paste" => Command::Paste,
        "find" => Command::Find,
        "replace" => Command::Replace,
        "findnext" => Command::FindNext,
        "selectall" => Command::SelectAll,
        "moveleft" | "left" => Command::MoveLeft,
        "moveright" | "right" => Command::MoveRight,
        "moveup" | "up" => Command::MoveUp,
        "movedown" | "down" => Command::MoveDown,
        "home" => Command::Home,
        "end" => Command::End,
        "pageup" => Command::PageUp,
        "pagedown" => Command::PageDown,
        "filestart" => Command::FileStart,
        "fileend" => Command::FileEnd,
        "backspace" => Command::Backspace,
        "delete" => Command::Delete,
        "newline" => Command::Newline,
        "tab" => Command::Tab,
        "addcursorabove" => Command::AddCursorAbove,
        "addcursorbelow" => Command::AddCursorBelow,
        "splitvertical" => Command::SplitVertical,
        "splithorizontal" => Command::SplitHorizontal,
        "nexttab" => Command::NextTab,
        "prevtab" => Command::PrevTab,
        "nextpane" => Command::NextPane,
        "toggletheme" => Command::ToggleTheme,
        "reloadplugins" => Command::ReloadPlugins,
        "listplugins" => Command::ListPlugins,
        "listkeybindings" => Command::ListKeybindings,
        "togglefold" | "fold" => Command::ToggleFold,
        "unfoldall" => Command::UnfoldAll,
        "help" => Command::Help,
        "cancel" => Command::Cancel,
        "confirm" => Command::Confirm,
        "none" => Command::None,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_descriptions() {
        let c = KeyModifiers::CONTROL;
        assert_eq!(parse_key("Ctrl-s"), Some((KeyCode::Char('s'), c)));
        assert_eq!(parse_key("Alt-p"), Some((KeyCode::Char('p'), KeyModifiers::ALT)));
        assert_eq!(
            parse_key("Ctrl-Shift-Left"),
            Some((KeyCode::Left, KeyModifiers::CONTROL | KeyModifiers::SHIFT))
        );
        assert_eq!(parse_key("F2"), Some((KeyCode::F(2), KeyModifiers::NONE)));
        assert_eq!(parse_key("nonsense"), None);
    }

    #[test]
    fn parse_command_names() {
        assert_eq!(parse_command("Save"), Some(Command::Save));
        assert_eq!(parse_command("find-next"), Some(Command::FindNext));
        assert_eq!(parse_command("select_all"), Some(Command::SelectAll));
        assert_eq!(parse_command("bogus"), None);
    }

    #[test]
    fn parse_key_sequences() {
        let seq = parse_sequence("Ctrl-x Ctrl-s").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].code, Some(KeyCode::Char('x')));
        assert!(seq[1].mods.contains(KeyModifiers::CONTROL));

        let any = parse_sequence("Alt-w any").unwrap();
        assert_eq!(any.len(), 2);
        assert_eq!(any[1].code, None);

        assert_eq!(parse_sequence("Ctrl-bogus").unwrap_or_default().len(), 0);
        assert!(parse_sequence("   ").is_none());
    }
}
