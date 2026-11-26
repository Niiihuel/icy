//! Line-local syntax tokenizer for the code viewer. Token kinds map to theme colors in TSX.
//! shortcut: no cross-line state (block comments / multi-line strings), upgrade to tree-sitter
//! if accuracy on those matters.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Function,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
}

/// A highlighted span using UTF-16 offsets into the line sent to JavaScript.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HighlightRange(pub usize, pub usize, pub TokenKind);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Rust,
    Script,
    Python,
    Shell,
    Toml,
    Plain,
}

/// Display name of the file's language for the editor footer ("Plain Text" when unknown).
pub fn language_name(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "Rust",
        "ts" => "TypeScript",
        "tsx" => "TypeScript JSX",
        "js" | "mjs" | "cjs" => "JavaScript",
        "jsx" => "JavaScript JSX",
        "json" => "JSON",
        "css" => "CSS",
        "go" => "Go",
        "c" | "h" => "C",
        "cpp" | "hpp" | "cc" => "C++",
        "java" => "Java",
        "kt" => "Kotlin",
        "swift" => "Swift",
        "cs" => "C#",
        "py" => "Python",
        "sh" | "bash" | "zsh" => "Shell Script",
        "fish" => "Fish",
        "yml" | "yaml" => "YAML",
        "toml" => "TOML",
        "ini" => "Ini",
        "md" => "Markdown",
        "html" => "HTML",
        _ => "Plain Text",
    }
}

pub fn lang_for_path(path: &str) -> Lang {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Lang::Rust,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "json" | "css" | "go" | "c" | "h" | "cpp"
        | "java" | "kt" | "swift" | "cs" => Lang::Script,
        "py" => Lang::Python,
        "sh" | "bash" | "zsh" | "fish" | "yml" | "yaml" => Lang::Shell,
        "toml" | "ini" => Lang::Toml,
        _ => Lang::Plain,
    }
}

fn keywords(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Rust => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
            "trait", "true", "type", "unsafe", "use", "where", "while",
        ],
        Lang::Script => &[
            "as",
            "async",
            "await",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "interface",
            "let",
            "new",
            "null",
            "of",
            "return",
            "static",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "yield",
            "func",
            "package",
            "struct",
        ],
        Lang::Python => &[
            "and", "as", "async", "await", "break", "class", "continue", "def", "elif", "else",
            "except", "False", "finally", "for", "from", "if", "import", "in", "is", "lambda",
            "None", "not", "or", "pass", "raise", "return", "True", "try", "while", "with",
            "yield",
        ],
        Lang::Shell => &[
            "case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if",
            "in", "local", "return", "set", "then", "while", "true", "false",
        ],
        Lang::Toml => &["true", "false"],
        Lang::Plain => &[],
    }
}

fn line_comment(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Rust | Lang::Script => &["//", "/*"],
        Lang::Python | Lang::Shell | Lang::Toml => &["#"],
        Lang::Plain => &[],
    }
}

fn push(out: &mut Vec<Token>, kind: TokenKind, text: &str) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(last) if last.kind == kind => last.text.push_str(text),
        _ => out.push(Token {
            kind,
            text: text.to_string(),
        }),
    }
}

pub fn tokenize(line: &str, lang: Lang) -> Vec<Token> {
    let mut out = Vec::new();
    scan(line, lang, |kind, start, end| {
        push(&mut out, kind, &line[start..end])
    });
    out
}

pub fn highlight_ranges(line: &str, lang: Lang) -> Vec<HighlightRange> {
    if lang == Lang::Plain {
        return Vec::new();
    }
    let mut ranges: Vec<HighlightRange> = Vec::new();
    let ascii = line.is_ascii();
    let (mut byte, mut utf16) = (0, 0);
    scan(line, lang, |kind, start, end| {
        if kind == TokenKind::Plain {
            return;
        }
        let (from, to) = if ascii {
            (start, end)
        } else {
            let from = utf16 + line[byte..start].encode_utf16().count();
            let to = from + line[start..end].encode_utf16().count();
            (byte, utf16) = (end, to);
            (from, to)
        };
        if let Some(last) = ranges
            .last_mut()
            .filter(|last| last.1 == from && last.2 == kind)
        {
            last.1 = to;
        } else {
            ranges.push(HighlightRange(from, to, kind));
        }
    });
    ranges
}

fn scan(line: &str, lang: Lang, mut emit: impl FnMut(TokenKind, usize, usize)) {
    if lang == Lang::Plain {
        emit(TokenKind::Plain, 0, line.len());
        return;
    }
    let kws = keywords(lang);
    let comments = line_comment(lang);
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &line[i..];
        if comments.iter().any(|c| rest.starts_with(c)) {
            emit(TokenKind::Comment, i, line.len());
            break;
        }
        let c = bytes[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            // Rust lifetimes ('a) are not strings.
            if c == b'\''
                && lang == Lang::Rust
                && !rest[1..].chars().nth(1).is_some_and(|n| n == '\'')
                && !rest.starts_with("'\\")
            {
                emit(TokenKind::Plain, i, i + 1);
                i += 1;
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = (j + 1).min(bytes.len());
            emit(TokenKind::String, i, end);
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'.' || bytes[j] == b'_')
            {
                j += 1;
            }
            emit(TokenKind::Number, i, j);
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &line[i..j];
            let kind = if kws.contains(&word) {
                TokenKind::Keyword
            } else if bytes.get(j) == Some(&b'(')
                || (bytes.get(j) == Some(&b'!') && lang == Lang::Rust)
            {
                TokenKind::Function
            } else if word.starts_with(|ch: char| ch.is_ascii_uppercase()) {
                TokenKind::Type
            } else {
                TokenKind::Plain
            };
            emit(kind, i, j);
            i = j;
            continue;
        }
        let ch_len = rest.chars().next().map_or(1, char::len_utf8);
        emit(TokenKind::Plain, i, i + ch_len);
        i += ch_len;
    }
}

/// Computes the 0-based `[start, end)` line range for a compact hover preview around `focus`.
pub fn preview_window(
    total: usize,
    focus: Option<usize>,
    max_lines: usize,
    context_before: usize,
) -> (usize, usize) {
    if total == 0 || max_lines == 0 {
        return (0, 0);
    }
    if total <= max_lines {
        return (0, total);
    }
    let Some(index) = focus else {
        return (0, max_lines);
    };
    let clamped = index.min(total - 1);
    let start = clamped
        .saturating_sub(context_before)
        .min(total - max_lines);
    (start, start + max_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str, lang: Lang) -> Vec<(TokenKind, String)> {
        tokenize(line, lang)
            .into_iter()
            .map(|t| (t.kind, t.text))
            .collect()
    }

    #[test]
    fn names_languages_for_the_footer() {
        assert_eq!(language_name("a.tsx"), "TypeScript JSX");
        assert_eq!(language_name("x.rs"), "Rust");
        assert_eq!(language_name("Makefile"), "Plain Text");
    }

    #[test]
    fn tokenizes_typescript_line() {
        use TokenKind::*;
        assert_eq!(
            kinds(r#"const x = foo("a\"b", 42); // done"#, Lang::Script),
            vec![
                (Keyword, "const".into()),
                (Plain, " x = ".into()),
                (Function, "foo".into()),
                (Plain, "(".into()),
                (String, r#""a\"b""#.into()),
                (Plain, ", ".into()),
                (Number, "42".into()),
                (Plain, "); ".into()),
                (Comment, "// done".into()),
            ]
        );
    }

    #[test]
    fn keeps_rust_lifetimes_and_types_apart() {
        let toks = kinds("fn f<'a>(s: &'a Str) -> Vec<u8>", Lang::Rust);
        assert!(toks.contains(&(TokenKind::Keyword, "fn".into())));
        assert!(toks.contains(&(TokenKind::Type, "Str".into())));
        assert!(!toks.iter().any(|(k, _)| *k == TokenKind::String));
        let rebuilt: String = toks.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(rebuilt, "fn f<'a>(s: &'a Str) -> Vec<u8>");
    }

    #[test]
    fn handles_unterminated_and_unicode() {
        let toks = tokenize("x = \"é", Lang::Python);
        let rebuilt: String = toks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(rebuilt, "x = \"é");
    }

    #[test]
    fn viewer_highlights_use_utf16_offsets() {
        use TokenKind::*;
        assert_eq!(
            highlight_ranges("😀 const x = \"ñ\"; // café", Lang::Script),
            [
                HighlightRange(3, 8, Keyword),
                HighlightRange(13, 16, String),
                HighlightRange(18, 25, Comment)
            ]
        );
        assert!(highlight_ranges("just text", Lang::Plain).is_empty());
    }

    #[test]
    fn computes_bounded_preview_windows() {
        assert_eq!(preview_window(0, None, 14, 3), (0, 0));
        assert_eq!(preview_window(10, Some(5), 0, 3), (0, 0));
        assert_eq!(preview_window(8, Some(4), 14, 3), (0, 8));
        assert_eq!(preview_window(100, None, 14, 3), (0, 14));
        assert_eq!(preview_window(100, Some(0), 14, 3), (0, 14));
        assert_eq!(preview_window(100, Some(41), 14, 3), (38, 52));
        assert_eq!(preview_window(100, Some(98), 14, 3), (86, 100));
        assert_eq!(preview_window(100, Some(500), 14, 3), (86, 100));
    }
}
