use anyhow::Result;
use rhai::{AST, Engine, Scope};

/// Minimal Rhai plugin host. Scripts may define hooks such as `fn on_save()`.
pub struct Plugins {
    engine: Engine,
    scope: Scope<'static>,
    asts: Vec<AST>,
}

impl Plugins {
    pub fn new() -> Self {
        Self { engine: Engine::new(), scope: Scope::new(), asts: Vec::new() }
    }

    /// Load a Rhai script and run it once (top-level statements).
    pub fn load_script(&mut self, path: &std::path::Path) -> Result<()> {
        let src = std::fs::read_to_string(path)?;
        let ast = self.engine.compile(&src)?;
        self.engine.run_ast(&ast)?;
        self.asts.push(ast);
        Ok(())
    }

    /// Call a no-arg hook defined by plugins, e.g. `on_save()`. Errors from
    /// scripts that do not define the hook are ignored on purpose.
    pub fn hook(&mut self, name: &str) -> Result<()> {
        for ast in &self.asts {
            let _ = self.engine.call_fn::<()>(&mut self.scope, ast, name, ());
        }
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> Result<rhai::Dynamic> {
        Ok(self.engine.eval_with_scope(&mut self.scope, code)?)
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
}
