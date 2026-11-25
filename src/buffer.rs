use ropey::Rope;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize, // byte offset within line
}

#[derive(Clone, Debug)]
pub enum Op {
    Insert { pos: usize, text: String },
    Delete { pos: usize, text: String },
}

use crate::undo::UndoStack;

pub struct Buffer {
    pub rope: Rope,
    pub cursor: Cursor,
    pub sel_anchor: Option<Cursor>,
    pub extra_cursors: Vec<Cursor>,
    pub dirty: bool,
    undo: UndoStack,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            cursor: Cursor::default(),
            sel_anchor: None,
            extra_cursors: Vec::new(),
            dirty: false,
            undo: UndoStack::default(),
        }
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line(&self, idx: usize) -> String {
        if idx >= self.line_count() {
            return String::new();
        }
        self.rope.line(idx).to_string().trim_end_matches(['\n', '\r']).to_string()
    }

    /// Number of visible characters in a line, excluding a trailing newline.
    fn line_len(&self, idx: usize) -> usize {
        if idx >= self.line_count() { return 0; }
        let l = self.rope.line(idx);
        let n = l.len_chars();
        let trailing = if n > 0 && l.char(n - 1) == '\n' {
            1 + if n > 1 && l.char(n - 2) == '\r' { 1 } else { 0 }
        } else { 0 };
        n - trailing
    }

    fn cursor_to_char(&self, c: Cursor) -> usize {
        let line = c.line.min(self.line_count().saturating_sub(1));
        let line_start = self.rope.line_to_char(line);
        line_start + c.col.min(self.line_len(line))
    }

    fn char_to_cursor(&self, ch: usize) -> Cursor {
        let line = self.rope.char_to_line(ch.min(self.rope.len_chars()));
        let start = self.rope.line_to_char(line);
        Cursor { line, col: ch.saturating_sub(start) }
    }

    pub fn insert_char(&mut self, ch: char) {
        let idx = self.cursor_to_char(self.cursor);
        let s = ch.to_string();
        self.rope.insert(idx, &s);
        self.undo.push(Op::Insert { pos: idx, text: s });
        self.cursor = self.char_to_cursor(idx + ch.len_utf8());
        self.dirty = true;
    }

    pub fn insert_str(&mut self, s: &str) {
        let idx = self.cursor_to_char(self.cursor);
        self.rope.insert(idx, s);
        self.undo.push(Op::Insert { pos: idx, text: s.to_string() });
        self.cursor = self.char_to_cursor(idx + s.chars().count());
        self.dirty = true;
    }

    pub fn newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn backspace(&mut self) {
        let idx = self.cursor_to_char(self.cursor);
        if idx == 0 {
            return;
        }
        let ch = self.rope.char(idx - 1);
        self.rope.remove(idx - 1..idx);
        self.undo.push(Op::Delete { pos: idx - 1, text: ch.to_string() });
        self.cursor = self.char_to_cursor(idx - 1);
        self.dirty = true;
    }

    pub fn delete_char(&mut self) {
        let idx = self.cursor_to_char(self.cursor);
        if idx >= self.rope.len_chars() {
            return;
        }
        let ch = self.rope.char(idx);
        self.rope.remove(idx..idx + 1);
        self.undo.push(Op::Delete { pos: idx, text: ch.to_string() });
        self.dirty = true;
    }

    /// Delete selection; returns true if there was a non-empty selection.
    pub fn delete_selection(&mut self) -> bool {
        if let Some(anchor) = self.sel_anchor {
            let a = self.cursor_to_char(anchor);
            let b = self.cursor_to_char(self.cursor);
            let (start, end) = (a.min(b), a.max(b));
            let text = self.rope.slice(start..end).to_string();
            let deleted = !text.is_empty();
            if deleted {
                self.rope.remove(start..end);
                self.undo.push(Op::Delete { pos: start, text });
                self.cursor = self.char_to_cursor(start);
                self.dirty = true;
            }
            self.sel_anchor = None;
            return deleted;
        }
        false
    }

    pub fn selected_text(&self) -> Option<String> {
        let anchor = self.sel_anchor?;
        let a = self.cursor_to_char(anchor);
        let b = self.cursor_to_char(self.cursor);
        if a == b {
            return None;
        }
        Some(self.rope.slice(a.min(b)..a.max(b)).to_string())
    }

    pub fn move_left(&mut self) {
        let idx = self.cursor_to_char(self.cursor);
        if idx > 0 {
            self.cursor = self.char_to_cursor(idx - 1);
        }
    }

    pub fn move_right(&mut self) {
        let idx = self.cursor_to_char(self.cursor);
        if idx < self.rope.len_chars() {
            self.cursor = self.char_to_cursor(idx + 1);
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor.line > 0 {
            let col = self.cursor.col;
            let line = self.cursor.line - 1;
            self.cursor = Cursor { line, col: col.min(self.line_len(line)) };
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor.line + 1 < self.line_count() {
            let col = self.cursor.col;
            let line = self.cursor.line + 1;
            self.cursor = Cursor { line, col: col.min(self.line_len(line)) };
        }
    }

    pub fn move_home(&mut self) {
        self.cursor.col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor.col = self.line_len(self.cursor.line);
    }

    pub fn move_bol(&mut self) { self.move_home(); }
    pub fn move_eol(&mut self) { self.move_end(); }

    pub fn move_file_start(&mut self) { self.cursor = Cursor::default(); }
    pub fn move_file_end(&mut self) {
        self.cursor = self.char_to_cursor(self.rope.len_chars());
    }

    pub fn undo(&mut self) {
        if let Some(op) = self.undo.pop_undo() {
            match op {
                Op::Insert { pos, text } => {
                    self.rope.remove(pos..pos + text.chars().count());
                    self.cursor = self.char_to_cursor(pos);
                }
                Op::Delete { pos, text } => {
                    self.rope.insert(pos, &text);
                    self.cursor = self.char_to_cursor(pos + text.chars().count());
                }
            }
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(op) = self.undo.pop_redo() {
            match op {
                Op::Insert { pos, text } => {
                    self.rope.insert(pos, &text);
                    self.cursor = self.char_to_cursor(pos + text.chars().count());
                }
                Op::Delete { pos, text } => {
                    self.rope.remove(pos..pos + text.chars().count());
                    self.cursor = self.char_to_cursor(pos);
                }
            }
            self.dirty = true;
        }
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_backspace_undo_redo() {
        let mut b = Buffer::new("hello");
        b.insert_char('X');
        assert!(b.to_string().starts_with('X'));
        b.undo();
        assert_eq!(b.to_string(), "hello");
        b.redo();
        assert_eq!(b.to_string(), "Xhello");
        b.backspace();
        assert_eq!(b.to_string(), "hello");
    }
    #[test]
    fn line_ops() {
        let mut b = Buffer::new("abc\ndef");
        b.move_down();
        b.move_end();
        b.insert_str("!");
        assert_eq!(b.to_string(), "abc\ndef!");
    }
}
