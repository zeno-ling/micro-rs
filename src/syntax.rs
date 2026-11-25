use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    ps: SyntaxSet,
    ts: ThemeSet,
    pub syntax_name: String,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            ps: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            syntax_name: "Plain Text".into(),
        }
    }

    pub fn for_file(path: &std::path::Path) -> Self {
        let mut h = Self::new();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if let Some(syn) = h.ps.find_syntax_by_extension(ext) {
            h.syntax_name = syn.name.clone();
        }
        h
    }

    pub fn theme(&self, name: &str) -> Theme {
        self.ts.themes.get(name).cloned().unwrap_or_else(|| {
            self.ts.themes["base16-ocean.dark"].clone()
        })
    }

    pub fn theme_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.ts.themes.keys().cloned().collect();
        v.sort();
        v
    }

    /// Highlight one line independently. Returns (rgb, text) regions.
    pub fn highlight(&self, line: &str, theme_name: &str) -> Vec<([u8; 3], String)> {
        let theme = self.theme(theme_name);
        let syn = self.ps.find_syntax_by_name(&self.syntax_name)
            .unwrap_or_else(|| self.ps.find_syntax_plain_text());
        let mut hl = HighlightLines::new(syn, &theme);
        match hl.highlight_line(line, &self.ps) {
            Ok(regions) => regions.into_iter().map(|(s, t)| {
                ([s.foreground.r, s.foreground.g, s.foreground.b], t.to_string())
            }).collect(),
            Err(_) => vec![([255, 255, 255], line.to_string())],
        }
    }
}
