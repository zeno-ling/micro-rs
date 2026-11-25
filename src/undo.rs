use crate::buffer::Op;

#[derive(Default)]
pub struct UndoStack {
    undo: Vec<Op>,
    redo: Vec<Op>,
}

impl UndoStack {
    pub fn push(&mut self, op: Op) {
        self.undo.push(op);
        self.redo.clear();
    }

    pub fn pop_undo(&mut self) -> Option<Op> {
        let op = self.undo.pop()?;
        self.redo.push(op.clone());
        Some(op)
    }

    pub fn pop_redo(&mut self) -> Option<Op> {
        let op = self.redo.pop()?;
        self.undo.push(op.clone());
        Some(op)
    }
}
