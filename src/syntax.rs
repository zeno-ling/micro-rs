//! Tree-sitter based syntax highlighting and code folding.
//!
//! Helix 25.07 switched to `tree-house` (a wrapper over tree-sitter) for
//! highlighting. We use tree-sitter directly instead: parse the whole file
//! into an AST, walk it and map each node to a semantic bucket (comment,
//! string, keyword, function, type, ...). Folding uses the row ranges of
//! multi-line AST nodes — the same source of truth Helix relies on. Doing it
//! directly (no highlight-query compilation) means no node-type drift across
//! grammar versions and full control over folding.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

pub type Rgb = [u8; 3];

/// A foldable region, expressed in zero-based *rows* (lines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fold {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Bucket {
    Plain,
    Comment,
    String,
    Keyword,
    Number,
    Function,
    Type,
    Attribute,
    Variable,
}

/// A theme is a colour per semantic bucket. Names mirror the old syntect
/// themes so existing `config.json` values keep working.
#[derive(Clone, Debug)]
pub struct Theme {
    buckets: HashMap<Bucket, Rgb>,
}

pub fn theme(name: &str) -> Theme {
    let mut m: HashMap<Bucket, Rgb> = [
        (Bucket::Plain, [201, 209, 217]),
        (Bucket::Comment, [92, 99, 112]),
        (Bucket::String, [152, 195, 121]),
        (Bucket::Keyword, [192, 97, 203]),
        (Bucket::Number, [209, 154, 102]),
        (Bucket::Function, [97, 175, 239]),
        (Bucket::Type, [229, 192, 123]),
        (Bucket::Attribute, [224, 108, 117]),
        (Bucket::Variable, [201, 209, 217]),
    ]
    .into_iter()
    .collect();
    match name {
        "base16-eighties.dark" => {
            m.insert(Bucket::Comment, [149, 150, 143]);
            m.insert(Bucket::Keyword, [220, 105, 103]);
            m.insert(Bucket::Function, [130, 170, 255]);
            m.insert(Bucket::Type, [203, 207, 62]);
            m.insert(Bucket::Attribute, [241, 76, 76]);
        }
        "InspiredGitHub" | "Inspired GitHub" => {
            m.insert(Bucket::Plain, [35, 36, 41]);
            m.insert(Bucket::Comment, [112, 113, 118]);
            m.insert(Bucket::String, [72, 118, 18]);
            m.insert(Bucket::Keyword, [173, 20, 87]);
            m.insert(Bucket::Function, [0, 92, 197]);
            m.insert(Bucket::Type, [119, 0, 154]);
        }
        "Solarized (dark)" | "base16-solarized.dark" => {
            m.insert(Bucket::Plain, [131, 148, 150]);
            m.insert(Bucket::Comment, [88, 110, 117]);
            m.insert(Bucket::String, [133, 153, 0]);
            m.insert(Bucket::Keyword, [211, 54, 130]);
            m.insert(Bucket::Function, [38, 139, 210]);
            m.insert(Bucket::Type, [181, 137, 0]);
            m.insert(Bucket::Attribute, [38, 139, 210]);
        }
        _ => {}
    }
    Theme { buckets: m }
}

pub fn theme_names() -> Vec<String> {
    vec![
        "base16-ocean.dark".into(),
        "base16-eighties.dark".into(),
        "InspiredGitHub".into(),
        "Solarized (dark)".into(),
    ]
}

const KEYWORDS: &[&str] = &[
    // Rust
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
    "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut",
    "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "type",
    "unsafe", "use", "where", "while", "true", "false",
    // Python
    "and", "assert", "class", "def", "del", "elif", "except", "finally", "from", "global",
    "import", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "try", "with",
    "yield", "None", "True", "False",
    // JS / TS / Go / C / C++ / shell
    "case", "catch", "default", "delete", "do", "export", "extends", "function", "instanceof",
    "new", "of", "switch", "this", "throw", "typeof", "var", "void", "null", "undefined",
    "interface", "namespace", "template", "typename", "virtual", "override", "nullptr",
    "chan", "defer", "fallthrough", "go", "map", "package", "range", "select", "auto",
    "register", "sizeof", "unsigned", "volatile", "then", "fi", "esac", "local", "export",
];

/// Classify a single AST node. `None` means "leave to child nodes / plain".
fn classify(kind: &str, text: &str) -> Option<Bucket> {
    if kind.contains("comment") { return Some(Bucket::Comment); }
    if kind.contains("string") || kind.contains("char_literal") || kind == "template_string" {
        return Some(Bucket::String);
    }
    if kind.contains("number") || kind.contains("integer") || kind.contains("float")
        || kind == "boolean_literal" {
        return Some(Bucket::Number);
    }
    if kind.contains("attribute") || kind.contains("decorator") || kind == "attribute_item" {
        return Some(Bucket::Attribute);
    }
    if KEYWORDS.contains(&kind) || KEYWORDS.contains(&text) { return Some(Bucket::Keyword); }
    if kind == "type_identifier" || kind == "type_spec" || kind.contains("type_identifier") {
        return Some(Bucket::Type);
    }
    None
}

struct Cache {
    hash: u64,
    theme: String,
    lines: Vec<Vec<(Rgb, String)>>,
    folds: Vec<Fold>,
}

fn hash_text(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Holds the tree-sitter grammar for one language plus a lazily refreshed
/// highlight/fold cache. Parsing happens on demand when the source changes.
pub struct Highlighter {
    language: Option<Language>,
    #[allow(dead_code)]
    pub syntax_name: String,
    cache: RefCell<Option<Cache>>,
}

impl Highlighter {
    #[allow(dead_code)]
    pub fn new() -> Self { Self::for_id("plaintext") }

    fn for_id(id: &'static str) -> Self {
        let language: Option<Language> = match id {
            "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
            "python" => Some(tree_sitter_python::LANGUAGE.into()),
            "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            "json" => Some(tree_sitter_json::LANGUAGE.into()),
            "c" => Some(tree_sitter_c::LANGUAGE.into()),
            "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            "bash" => Some(tree_sitter_bash::LANGUAGE.into()),
            "html" => Some(tree_sitter_html::LANGUAGE.into()),
            "css" => Some(tree_sitter_css::LANGUAGE.into()),
            "markdown" => Some(tree_sitter_md::LANGUAGE.into()),
            _ => None,
        };
        Self { language, syntax_name: display_name(id).into(), cache: RefCell::new(None) }
    }

    pub fn for_file(path: &Path) -> Self { Self::for_id(lang_id_for_path(path)) }

    #[allow(dead_code)]
    pub fn theme(&self, name: &str) -> Theme { theme(name) }
    pub fn theme_names(&self) -> Vec<String> { theme_names() }

    /// API compatibility with the old syntect highlighter.
    #[allow(dead_code)]
    pub fn highlight(&self, line: &str, theme_name: &str) -> Vec<(Rgb, String)> {
        let th = theme(theme_name);
        vec![(th.buckets[&Bucket::Plain], line.to_string())]
    }

    fn ensure_cache(&self, full_text: &str, theme_name: &str) {
        let h = hash_text(full_text);
        if let Some(c) = self.cache.borrow().as_ref() {
            if c.hash == h && c.theme == theme_name { return; }
        }
        let computed = self.compute(full_text, theme_name);
        *self.cache.borrow_mut() = Some(computed);
    }

    /// Highlight spans for a single 0-based line, given the full source text.
    pub fn spans_for_line(&self, line: usize, full_text: &str, theme_name: &str) -> Vec<(Rgb, String)> {
        self.ensure_cache(full_text, theme_name);
        let cache = self.cache.borrow();
        cache.as_ref().unwrap().lines.get(line).cloned().unwrap_or_default()
    }

    pub fn folds(&self, full_text: &str, theme_name: &str) -> Vec<Fold> {
        self.ensure_cache(full_text, theme_name);
        self.cache.borrow().as_ref().unwrap().folds.clone()
    }

    pub fn fold_at(&self, row: usize, full_text: &str, theme_name: &str) -> Option<Fold> {
        self.folds(full_text, theme_name).into_iter().find(|f| f.start == row)
    }

    fn compute(&self, full_text: &str, theme_name: &str) -> Cache {
        let th = theme(theme_name);
        let plain = th.buckets[&Bucket::Plain];
        let line_count = full_text.lines().count().max(1);
        // Per-line, per-byte colour. Later (deeper/smaller) nodes overwrite
        // earlier ones, so keyword/name nodes win over enclosing containers.
        let mut colors: Vec<Vec<Option<Rgb>>> =
            (0..=line_count).map(|i| vec![None; full_text.lines().nth(i).map(|l| l.len() + 1).unwrap_or(1)]).collect();
        let mut folds = Vec::new();

        if let Some(lang) = &self.language {
            let mut parser = Parser::new();
            if parser.set_language(lang).is_ok() {
                if let Some(tree) = parser.parse(full_text, None) {
                    let root = tree.root_node();
                    collect_folds(root, &mut folds);
                    paint_ast(root, full_text, &mut colors, &th);
                }
            }
        }

        let lines = full_text.lines().enumerate().map(|(i, text)| {
            build_line(text.to_string(), plain, colors.get(i).map(|v| v.as_slice()).unwrap_or(&[]))
        }).collect();
        Cache { hash: hash_text(full_text), theme: theme_name.to_string(), lines, folds }
    }
}

/// Colour the named parts of function/class definitions before the generic
/// walk, so a function name reads as Function and a class name as Type.
fn paint_definition_names(node: Node<'_>, source: &str, colors: &mut [Vec<Option<Rgb>>], th: &Theme) {
    let kind = node.kind();
    let field = if matches!(kind,
        "function_item" | "function_signature_item" | "function_definition"
        | "method_definition" | "function_declaration" | "method_declaration") {
        Some((Bucket::Function, "name"))
    } else if matches!(kind,
        "struct_item" | "enum_item" | "trait_item" | "class_definition"
        | "class_declaration" | "interface_declaration" | "type_spec") {
        Some((Bucket::Type, "name"))
    } else { None };
    if let Some((bucket, field_name)) = field {
        if let Some(name) = node.child_by_field_name(field_name) {
            paint_range(source, colors, name.start_byte(), name.end_byte(), th.buckets[&bucket]);
        }
    }
}

fn paint_ast(node: Node<'_>, source: &str, colors: &mut [Vec<Option<Rgb>>], th: &Theme) {
    paint_definition_names(node, source, colors, th);
    let kind = node.kind();
    if let Some(bucket) = classify(kind, &source[node.start_byte().min(source.len())..node.end_byte().min(source.len())]) {
        paint_range(source, colors, node.start_byte(), node.end_byte(), th.buckets[&bucket]);
    }
    let mut i = 0;
    while let Some(child) = node.child(i) {
        paint_ast(child, source, colors, th);
        i += 1;
    }
}

fn paint_range(text: &str, colors: &mut [Vec<Option<Rgb>>], start: usize, end: usize, colour: Rgb) {
    let len = text.len();
    let s = start.min(len);
    let e = end.min(len);
    let mut pos = s;
    while pos < e {
        let line = text[..pos].bytes().filter(|&b| b == b'\n').count();
        let line_start = text[..pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let next_nl = text[pos..e].find('\n').map(|p| pos + p);
        let line_end = next_nl.unwrap_or(e);
        // AST byte offsets are absolute; colour rows are per-line.
        let col_start = pos - line_start;
        let col_end = line_end - line_start;
        if let Some(row) = colors.get_mut(line) {
            for c in col_start..col_end.min(row.len()) { row[c] = Some(colour); }
        }
        pos = next_nl.map(|n| n + 1).unwrap_or(e);
    }
}

fn build_line(text: String, plain: Rgb, colors: &[Option<Rgb>]) -> Vec<(Rgb, String)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let colour = colors.get(i).copied().flatten();
        let mut j = i + 1;
        while j < bytes.len() && colors.get(j).copied().flatten() == colour { j += 1; }
        let piece = std::str::from_utf8(&bytes[i..j]).unwrap_or("");
        spans.push((colour.unwrap_or(plain), piece.to_string()));
        i = j;
    }
    if spans.is_empty() { spans.push((plain, text)); }
    spans
}

fn foldable_kind(kind: &str) -> bool {
    // `source_file` is deliberately absent: it spans the whole file.
    matches!(kind,
        "function_item" | "function_signature_item" | "impl_item" | "struct_item"
        | "enum_item" | "trait_item" | "mod_item" | "block" | "match_expression"
        | "function_definition" | "class_definition" | "decorated_definition"
        | "if_statement" | "for_statement" | "while_statement" | "with_statement"
        | "method_definition" | "class_declaration" | "function_declaration"
        | "arrow_function" | "object" | "array" | "interface_declaration"
        | "struct_specifier" | "class_specifier" | "function_declarator"
        | "method_declaration" | "type_spec" | "dictionary" | "list"
    )
}

fn collect_folds(node: Node<'_>, folds: &mut Vec<Fold>) {
    let sr = node.start_position().row;
    let er = node.end_position().row;
    if er > sr && foldable_kind(node.kind()) {
        folds.push(Fold { start: sr, end: er, label: node.kind().to_string() });
    }
    let mut i = 0;
    while let Some(child) = node.child(i) {
        collect_folds(child, folds);
        i += 1;
    }
}

fn lang_id_for_path(path: &Path) -> &'static str {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.eq_ignore_ascii_case("makefile") { return "bash"; }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" | "pyw" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "json" | "jsonc" => "json",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "go" => "go",
        "sh" | "bash" | "zsh" => "bash",
        "html" | "htm" | "vue" => "html",
        "css" | "scss" => "css",
        "md" | "markdown" => "markdown",
        _ => "plaintext",
    }
}

fn display_name(id: &str) -> &'static str {
    match id {
        "rust" => "Rust", "python" => "Python", "javascript" => "JavaScript",
        "typescript" => "TypeScript", "tsx" => "TSX", "json" => "JSON",
        "c" => "C", "cpp" => "C++", "go" => "Go", "bash" => "Shell",
        "html" => "HTML", "css" => "CSS", "markdown" => "Markdown",
        _ => "Plain Text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_rust_keywords_and_comments() {
        let src = "fn main() {\n    let x = 1; // hi\n}\n";
        let h = Highlighter::for_id("rust");
        let kw = theme("base16-ocean.dark").buckets[&Bucket::Keyword];
        let cm = theme("base16-ocean.dark").buckets[&Bucket::Comment];
        let colours = h.spans_for_line(0, src, "base16-ocean.dark");
        assert!(colours.iter().any(|(c, t)| *c == kw && t.trim() == "fn"), "{colours:?}");
        let comment = h.spans_for_line(1, src, "base16-ocean.dark");
        assert!(comment.iter().any(|(c, t)| *c == cm && t.contains("hi")), "{comment:?}");
    }

    #[test]
    fn folds_multiline_rust_function() {
        let src = "fn main() {\n    let x = 1;\n}\n";
        let h = Highlighter::for_id("rust");
        let f = h.folds(src, "base16-ocean.dark");
        assert!(f.iter().any(|x| x.start == 0 && x.end == 2), "{f:?}");
    }

    #[test]
    fn all_languages_parse() {
        for id in ["rust", "python", "javascript", "typescript", "tsx", "json",
                   "c", "cpp", "go", "bash", "html", "css", "markdown"] {
            let h = Highlighter::for_id(id);
            assert!(h.language.is_some(), "no grammar for {id}");
            let src = "x = 1\n";
            let _ = h.spans_for_line(0, src, "base16-ocean.dark");
        }
    }

    #[test]
    fn language_detection() {
        assert_eq!(lang_id_for_path(Path::new("a.rs")), "rust");
        assert_eq!(lang_id_for_path(Path::new("a.py")), "python");
        assert_eq!(lang_id_for_path(Path::new("Makefile")), "bash");
        assert_eq!(lang_id_for_path(Path::new("a.unknown")), "plaintext");
    }
}
