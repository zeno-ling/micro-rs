use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;

use crate::commands::Command;

/// One key in a binding sequence. `code == None` means "any key", which is how
/// official micro exposes `RegisterKeyAnyBinding`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPattern {
    pub code: Option<KeyCode>,
    pub mods: KeyModifiers,
    /// When true the incoming modifiers must equal `mods` exactly; otherwise
    /// `mods` only has to be a subset (so `Alt-p` also fires for `Alt-Shift-p`
    /// only when this is false).
    pub exact_mods: bool,
}

impl KeyPattern {
    pub fn exact(code: KeyCode, mods: KeyModifiers) -> Self {
        Self { code: Some(code), mods, exact_mods: true }
    }

    pub fn any() -> Self {
        Self { code: None, mods: KeyModifiers::NONE, exact_mods: false }
    }

    pub fn matches(&self, code: KeyCode, mods: KeyModifiers) -> bool {
        if let Some(want) = self.code {
            if want != code {
                return false;
            }
        }
        if self.exact_mods {
            mods == self.mods
        } else {
            mods.contains(self.mods)
        }
    }
}

/// A binding only applies while the named mode has the required state.
/// Mirrors micro's `action.ModeConstraint{Modes []Mode, Disabled bool}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeConstraint {
    pub mode: String,
    /// When true the binding is active only while the mode is *off*; this is
    /// micro's "disabled mode" form used for bindings that should switch a mode
    /// on rather than act inside it.
    pub disabled: bool,
}

impl ModeConstraint {
    pub fn active_in(mode: &str) -> Self {
        Self { mode: mode.to_string(), disabled: false }
    }

    /// The "mode off" form; unused by the defaults but exposed so config and
    /// plugins can express micro's disabled-mode bindings.
    #[allow(dead_code)]
    pub fn inactive_in(mode: &str) -> Self {
        Self { mode: mode.to_string(), disabled: true }
    }}

#[derive(Clone, Debug)]
struct BoundAction {
    cmd: Command,
    modes: Vec<ModeConstraint>,
}

#[derive(Clone, Debug, Default)]
struct Node {
    children: Vec<(KeyPattern, Node)>,
    actions: Vec<BoundAction>,
}

impl Node {
    /// Exact patterns win over wildcard patterns for the same key press.
    fn child_for(&self, code: KeyCode, mods: KeyModifiers) -> Option<&Node> {
        let mut wildcard: Option<&Node> = None;
        for (pat, node) in &self.children {
            if !pat.matches(code, mods) {
                continue;
            }
            if pat.code.is_some() {
                return Some(node);
            }
            if wildcard.is_none() {
                wildcard = Some(node);
            }
        }
        wildcard
    }
}

/// What a key press produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Nothing bound to this key press.
    None,
    /// The press is a prefix of a longer sequence; wait for more keys.
    Pending,
    /// A binding fired.
    Command(Command),
}

/// Multi-key binding table: children are a trie of key sequences, each node may
/// hold several actions guarded by mode constraints.
#[derive(Clone, Debug)]
pub struct KeyTree {
    root: Node,
    /// Keys already consumed by a partially-matched sequence.
    pending: Vec<(KeyCode, KeyModifiers)>,
    modes: HashMap<String, bool>,
}

impl Default for KeyTree {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyTree {
    pub fn new() -> Self {
        let mut modes = HashMap::new();
        // The editing mode is on by default so plain bindings work out of the box.
        modes.insert("buffer".to_string(), true);
        modes.insert("prompt".to_string(), false);
        modes.insert("menu".to_string(), false);
        Self { root: Node::default(), pending: Vec::new(), modes }
    }

    pub fn set_mode(&mut self, mode: &str, on: bool) {
        self.modes.insert(mode.to_string(), on);
    }

    pub fn mode(&self, mode: &str) -> bool {
        self.modes.get(mode).copied().unwrap_or(false)
    }

    /// Number of keys waiting for the rest of a sequence.
    #[allow(dead_code)]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Human-readable rendering of the half-typed sequence, for the status bar.
    pub fn pending_label(&self) -> String {
        self.pending
            .iter()
            .map(|(code, mods)| describe_key(*code, *mods))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[allow(dead_code)]
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }

    /// Register `cmd` for the key sequence `seq`, guarded by `modes`.
    pub fn register(&mut self, seq: &[KeyPattern], cmd: Command, modes: Vec<ModeConstraint>) {
        if seq.is_empty() {
            return;
        }
        let mut node = &mut self.root;
        for pat in seq {
            let pos = node
                .children
                .iter()
                .position(|(p, _)| p.code == pat.code && p.mods == pat.mods && p.exact_mods == pat.exact_mods);
            let idx = match pos {
                Some(i) => i,
                None => {
                    node.children.push((*pat, Node::default()));
                    node.children.len() - 1
                }
            };
            node = &mut node.children[idx].1;
        }
        // Replace an existing binding for the same key sequence + mode set.
        let existing = node.actions.iter().position(|a| a.modes == modes);
        let action = BoundAction { cmd, modes };
        match existing {
            Some(i) => node.actions[i] = action,
            None => node.actions.push(action),
        }
    }

    /// Register a binding that fires for any key press without modifiers.
    #[allow(dead_code)]
    pub fn register_any(&mut self, cmd: Command, modes: Vec<ModeConstraint>) {
        self.register(&[KeyPattern::any()], cmd, modes);
    }

    fn action_enabled(&self, action: &BoundAction) -> bool {
        action
            .modes
            .iter()
            .all(|m| self.mode(&m.mode) != m.disabled)
    }

    fn first_enabled(&self, node: &Node) -> Option<Command> {
        node.actions
            .iter()
            .find(|a| self.action_enabled(a))
            .map(|a| a.cmd.clone())
    }

    /// Feed one key press into the trie and report what happened.
    ///
    /// Semantics follow micro's `(*KeyTree).NextEvent`: walking deeper returns
    /// Pending, reaching a leaf with an enabled action fires it, and a dead end
    /// falls back to any action registered on the current node.
    pub fn next_event(&mut self, code: KeyCode, mods: KeyModifiers) -> KeyOutcome {
        // Locate the node for the keys already consumed.
        let mut node = &self.root;
        for (pc, pm) in &self.pending {
            match node.child_for(*pc, *pm) {
                Some(next) => node = next,
                None => {
                    self.pending.clear();
                    node = &self.root;
                    break;
                }
            }
        }

        let Some(child) = node.child_for(code, mods) else {
            let fallback = self.first_enabled(node);
            self.pending.clear();
            return match fallback {
                Some(cmd) => KeyOutcome::Command(cmd),
                None => KeyOutcome::None,
            };
        };

        if child.actions.is_empty() && !child.children.is_empty() {
            self.pending.push((code, mods));
            return KeyOutcome::Pending;
        }

        let fired = self.first_enabled(child);
        self.pending.clear();
        match fired {
            Some(cmd) if child.children.is_empty() => KeyOutcome::Command(cmd),
            Some(cmd) => {
                // A node can be both a complete binding and a prefix; micro
                // prefers the longer sequence, so stay pending when one exists.
                self.pending.push((code, mods));
                let _ = cmd;
                KeyOutcome::Pending
            }
            None if !child.children.is_empty() => {
                self.pending.push((code, mods));
                KeyOutcome::Pending
            }
            None => KeyOutcome::None,
        }
    }

    /// Every registered binding, sorted, for `help keybindings` style listings.
    pub fn bindings(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut stack: Vec<(&Node, Vec<String>)> = vec![(&self.root, Vec::new())];
        while let Some((node, path)) = stack.pop() {
            for action in &node.actions {
                out.push((path.join(" "), format!("{:?}", action.cmd)));
            }
            for (pat, child) in &node.children {
                let mut p = path.clone();
                let label = match pat.code {
                    Some(code) => describe_key(code, pat.mods),
                    None => "any".to_string(),
                };
                p.push(label);
                stack.push((child, p));
            }
        }
        out.sort();
        out
    }
}

/// Render a key press the way micro names it in `bindings.json`.
pub fn describe_key(code: KeyCode, mods: KeyModifiers) -> String {
    let mut out = String::new();
    if mods.contains(KeyModifiers::CONTROL) {
        out.push_str("Ctrl-");
    }
    if mods.contains(KeyModifiers::ALT) {
        out.push_str("Alt-");
    }
    if mods.contains(KeyModifiers::SHIFT) {
        out.push_str("Shift-");
    }
    let name = match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    };
    out.push_str(&name);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(tree: &mut KeyTree, seq: &str, cmd: Command) {
        let pats = crate::commands::parse_sequence(seq).expect("valid sequence");
        tree.register(&pats, cmd, vec![]);
    }

    #[test]
    fn single_key_binding_fires() {
        let mut t = KeyTree::new();
        bind(&mut t, "Ctrl-s", Command::Save);
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::Command(Command::Save)
        );
        assert_eq!(t.pending_len(), 0);
    }

    #[test]
    fn multi_key_sequence_needs_prefix() {
        let mut t = KeyTree::new();
        bind(&mut t, "Ctrl-x Ctrl-s", Command::Save);
        assert_eq!(t.next_event(KeyCode::Char('x'), KeyModifiers::CONTROL), KeyOutcome::Pending);
        assert_eq!(t.pending_len(), 1);
        assert_eq!(t.pending_label(), "Ctrl-x");
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::Command(Command::Save)
        );
        assert_eq!(t.pending_len(), 0);
    }

    #[test]
    fn dead_end_clears_pending() {
        let mut t = KeyTree::new();
        bind(&mut t, "Ctrl-x Ctrl-s", Command::Save);
        bind(&mut t, "Ctrl-q", Command::Quit);
        assert_eq!(t.next_event(KeyCode::Char('x'), KeyModifiers::CONTROL), KeyOutcome::Pending);
        // 'q' with Ctrl is not under the Ctrl-x prefix, so nothing fires.
        assert_eq!(t.next_event(KeyCode::Char('q'), KeyModifiers::CONTROL), KeyOutcome::None);
        assert_eq!(t.pending_len(), 0);
    }

    #[test]
    fn wildcard_and_exact_precedence() {
        let mut t = KeyTree::new();
        t.register_any(Command::Insert('?'), vec![]);
        bind(&mut t, "Ctrl-s", Command::Save);
        // Exact binding wins over the wildcard.
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::Command(Command::Save)
        );
        // Unbound key falls through to the wildcard.
        assert_eq!(
            t.next_event(KeyCode::Char('k'), KeyModifiers::NONE),
            KeyOutcome::Command(Command::Insert('?'))
        );
    }

    #[test]
    fn mode_constraint_gates_binding() {
        let mut t = KeyTree::new();
        t.register(
            &crate::commands::parse_sequence("Ctrl-s").unwrap(),
            Command::Save,
            vec![ModeConstraint::active_in("buffer")],
        );
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::Command(Command::Save)
        );
        t.set_mode("buffer", false);
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::None
        );
    }

    #[test]
    fn user_binding_overrides_default() {
        let mut t = KeyTree::new();
        bind(&mut t, "Ctrl-s", Command::Save);
        bind(&mut t, "Ctrl-s", Command::Quit);
        assert_eq!(
            t.next_event(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyOutcome::Command(Command::Quit)
        );
    }
}
