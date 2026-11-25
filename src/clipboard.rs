use copypasta::{ClipboardContext, ClipboardProvider};

pub struct Clipboard {
    ctx: Option<ClipboardContext>,
    internal: String,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    pub fn new() -> Self {
        let ctx = ClipboardContext::new().ok();
        Self { ctx, internal: String::new() }
    }

    pub fn set(&mut self, text: &str) {
        self.internal = text.to_string();
        if let Some(ctx) = &mut self.ctx {
            let _ = ctx.set_contents(text.to_string());
        }
    }

    pub fn get(&mut self) -> String {
        if let Some(ctx) = &mut self.ctx {
            if let Ok(s) = ctx.get_contents() {
                if !s.is_empty() {
                    return s;
                }
            }
        }
        self.internal.clone()
    }
}
