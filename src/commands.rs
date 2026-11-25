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
    None,
}

pub fn default_keymap() -> std::collections::HashMap<(KeyCode, KeyModifiers), Command> {
    use KeyCode::*;
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
    m
}
