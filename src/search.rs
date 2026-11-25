use regex::Regex;

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub regex: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self { case_sensitive: false, regex: false }
    }
}

/// Find all matches in `text`; returns (byte_start, byte_end) pairs.
pub fn find_all(text: &str, needle: &str, opts: &SearchOptions) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    if opts.regex {
        let pattern = if opts.case_sensitive {
            needle.to_string()
        } else {
            format!("(?i){needle}")
        };
        return match Regex::new(&pattern) {
            Ok(re) => re.find_iter(text).map(|m| (m.start(), m.end())).collect(),
            Err(_) => Vec::new(),
        };
    }
    let (haystack, needle) = if opts.case_sensitive {
        (text.to_string(), needle.to_string())
    } else {
        (text.to_lowercase(), needle.to_lowercase())
    };
    let mut out = Vec::new();
    let mut off = 0;
    while let Some(rel) = haystack[off..].find(&needle) {
        let start = off + rel;
        out.push((start, start + needle.len()));
        off = start + needle.len().max(1);
        if off >= haystack.len() { break; }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_case_insensitive() {
        let r = find_all("Hello hello HELLO", "hello", &SearchOptions::default());
        assert_eq!(r, vec![(0, 5), (6, 11), (12, 17)]);
    }
    #[test]
    fn regex_mode() {
        let r = find_all("a1 b22 c333", r"\d+", &SearchOptions { regex: true, ..Default::default() });
        assert_eq!(r, vec![(1, 2), (4, 6), (8, 11)]);
    }
}
