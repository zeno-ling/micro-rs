use anyhow::Result;
use rhai::{AST, Dynamic, Engine, Scope};

/// Rhai plugin host. Scripts are loaded from `~/.config/micro-rs/plugins/*.rhai`.
/// Each script may define hooks such as `fn on_save()` or `fn on_any_event(event)`.
pub struct Plugins {
    engine: Engine,
    scope: Scope<'static>,
    asts: Vec<AST>,
}

impl Plugins {
    pub fn new() -> Self {
        Self { engine: Engine::new(), scope: Scope::new(), asts: Vec::new() }
    }

    /// Load a Rhai script and run its top-level statements once.
    pub fn load_script(&mut self, path: &std::path::Path) -> Result<()> {
        let src = std::fs::read_to_string(path)?;
        let ast = self.engine.compile(&src)?;
        self.engine.run_ast(&ast)?;
        self.asts.push(ast);
        Ok(())
    }

    /// Call a named hook in every loaded plugin, passing `args` to each.
    /// Plugins that do not define the hook are silently skipped.
    /// Returns the last value produced by any plugin.
    pub fn hook(&mut self, name: &str, args: Vec<Dynamic>) -> Option<Dynamic> {
        let mut last = None;
        for ast in &self.asts {
            if let Ok(v) = self.engine.call_fn::<Dynamic>(&mut self.scope, ast, name, args.clone()) {
                last = Some(v);
            }
        }
        last
    }

    /// Hook whose `true` return from any plugin means "block this action".
    /// Used for intercepting hooks such as `onBeforeTextEvent`.
    #[allow(dead_code)] // kept for plugins that still use the plain-string form
    pub fn hook_bool(&mut self, name: &str, args: Vec<Dynamic>) -> bool {
        let mut blocked = false;
        for ast in &self.asts {
            if let Ok(v) = self.engine.call_fn::<Dynamic>(&mut self.scope, ast, name, args.clone()) {
                if let Ok(b) = v.as_bool() {
                    blocked |= b;
                }
            }
        }
        blocked
    }

    /// Fire a hook with a structured argument map, e.g.
    /// `#{event: "Insert", path: "a.txt", line: 12}`.
    ///
    /// Official micro hands plugins an `Event` object whose fields differ per
    /// hook; a map is the closest Rhai equivalent and it lets a plugin both read
    /// context and return a replacement value.
    pub fn hook_map(&mut self, name: &str, map: rhai::Map) -> Option<Dynamic> {
        self.hook(name, vec![Dynamic::from_map(map)])
    }

    /// Structured intercepting hook. A plugin blocks the action by returning
    /// `true`, `#{block: true}` or a map containing `block: true`.
    pub fn hook_map_bool(&mut self, name: &str, map: rhai::Map) -> bool {
        let mut blocked = false;
        for ast in &self.asts {
            let Ok(v) = self.engine.call_fn::<Dynamic>(&mut self.scope, ast, name, vec![Dynamic::from_map(map.clone())]) else {
                continue;
            };
            if let Ok(b) = v.as_bool() {
                blocked |= b;
                continue;
            }
            if v.is_map() {
                let m = v.cast::<rhai::Map>();
                if let Some(b) = m.get("block").and_then(|x| x.clone().try_cast::<bool>()) {
                    blocked |= b;
                }
            }
        }
        blocked
    }

    #[allow(dead_code)]
    pub fn eval(&mut self, code: &str) -> Result<Dynamic> {
        Ok(self.engine.eval_with_scope(&mut self.scope, code)?)
    }

    /// Number of successfully loaded plugin scripts.
    pub fn count(&self) -> usize {
        self.asts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eval_and_state() {
        let mut p = Plugins::new();
        let v = p.eval("let x = 2; x * 3").unwrap();
        assert_eq!(v.as_int().unwrap(), 6);
    }

    #[test]
    fn structured_hook_passes_map_and_reads_verdict() {
        let mut p = Plugins::new();
        let src = r#"
            fn onBeforeTextEvent(ev) {
                if ev.event == "Insert" && ev.text == "x" { return #{ block: true }; }
                return #{ block: false };
            }
            fn onSave(ev) { return ev.path; }
        "#;
        let ast = p.engine.compile(src).unwrap();
        p.engine.run_ast(&ast).unwrap();
        p.asts.push(ast);

        let mut m = rhai::Map::new();
        m.insert("event".into(), Dynamic::from("Insert".to_string()));
        m.insert("text".into(), Dynamic::from("x".to_string()));
        assert!(p.hook_map_bool("onBeforeTextEvent", m));

        let mut m2 = rhai::Map::new();
        m2.insert("event".into(), Dynamic::from("Insert".to_string()));
        m2.insert("text".into(), Dynamic::from("y".to_string()));
        assert!(!p.hook_map_bool("onBeforeTextEvent", m2));

        let mut m3 = rhai::Map::new();
        m3.insert("path".into(), Dynamic::from("a.txt".to_string()));
        let r = p.hook_map("onSave", m3).unwrap();
        assert_eq!(r.into_string().unwrap(), "a.txt");
    }

    #[test]
    fn hook_with_args_and_intercept() {
        let mut p = Plugins::new();
        let src = r#"
            fn onBeforeTextEvent(event) { return event == "blocked"; }
            fn onAnyEvent(event) { return event; }
        "#;
        let ast = p.engine.compile(src).unwrap();
        p.engine.run_ast(&ast).unwrap();
        p.asts.push(ast);

        assert!(p.hook_bool("onBeforeTextEvent", vec![Dynamic::from("blocked".to_string())]));
        assert!(!p.hook_bool("onBeforeTextEvent", vec![Dynamic::from("Insert".to_string())]));
        let r = p.hook("onAnyEvent", vec![Dynamic::from("Save".to_string())]);
        assert!(r.is_some());
        assert_eq!(r.unwrap().into_string().unwrap(), "Save");
    }
}
