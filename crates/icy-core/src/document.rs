use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use serde::Serialize;

use crate::syntax::language_name;

pub const MAX_VIEW_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_VIEW_LINES: usize = 100_000;
pub const MAX_EDIT_BYTES: usize = 64 * 1024;
const MAX_EDIT_LINES: usize = 1500;
const MAX_EDIT_LINE_BYTES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Indent {
    Tabs,
    Spaces { size: u8 },
}

/// Facts about a text file shown in the editor footer; computed once per load on the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextInfo {
    pub language: &'static str,
    pub bom: bool,
    pub crlf: bool,
    pub indent: Option<Indent>,
}

pub fn text_info(path: &str, content: &str) -> TextInfo {
    TextInfo {
        language: language_name(path),
        bom: content.starts_with('\u{feff}'),
        // The first line break decides, as the editor's save path assumes one style.
        crlf: content
            .find('\n')
            .is_some_and(|i| content[..i].ends_with('\r')),
        indent: detect_indent(content),
    }
}

/// Guesses the indentation from the first 1000 lines: tab-led lines against the most common
/// space step (1..=8). Blank lines and JSDoc continuations (`*`) are ignored.
pub fn detect_indent(content: &str) -> Option<Indent> {
    let mut tabs = 0usize;
    let mut steps = [0usize; 9];
    let mut previous = 0usize;
    for line in content.lines().take(1000) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }
        if line.starts_with('\t') {
            tabs += 1;
            previous = 0;
            continue;
        }
        let width = line.len() - trimmed.len();
        if width > previous && width - previous <= 8 {
            steps[width - previous] += 1;
        }
        previous = width;
    }
    let total: usize = steps.iter().sum();
    if tabs > total {
        return Some(Indent::Tabs);
    }
    let (size, count) = (1..=8)
        .map(|s| (s, steps[s]))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))?;
    (count > 0).then_some(Indent::Spaces { size: size as u8 })
}

pub fn read_regular(path: &Path, max: usize) -> io::Result<Vec<u8>> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::other("Not a regular file."));
    }
    if metadata.len() > max as u64 {
        return Err(io::Error::other(
            "File is too large for the built-in viewer.",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(io::Error::other(
            "File is too large for the built-in viewer.",
        ));
    }
    Ok(bytes)
}

pub fn text(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    (!text.contains('\0')).then_some(text)
}

pub fn editable(text: &str) -> Result<(), &'static str> {
    if text.len() > MAX_EDIT_BYTES
        || text.lines().count() > MAX_EDIT_LINES
        || text.lines().any(|line| line.len() > MAX_EDIT_LINE_BYTES)
    {
        return Err("This file is too large for inline editing. Use Open in…");
    }
    // JSNIR's multiline TextInput rewrites tabs and other control characters. Until it
    // exposes a lossless source-editor mode, don't allow an edit that would change them.
    if text.chars().any(|ch| {
        (ch.is_control() && !matches!(ch, '\r' | '\n')) || matches!(ch, '\u{2028}' | '\u{2029}')
    }) {
        return Err("This file contains tabs or special whitespace. Use Open in… to preserve it.");
    }
    let mut endings = text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| line.ends_with("\r\n"));
    if endings
        .next()
        .is_some_and(|first| endings.any(|ending| ending != first))
        || text
            .split_inclusive("\r\n")
            .any(|line| line.strip_suffix("\r\n").unwrap_or(line).contains('\r'))
    {
        return Err("Mixed or unsupported line endings. Use Open in…");
    }
    Ok(())
}

/// Replace only the version the editor opened. Preserve a UTF-8 BOM, CRLF and permissions.
pub fn save(path: &Path, expected: &str, draft: &str) -> io::Result<String> {
    if expected.len() > MAX_EDIT_BYTES
        || draft.len() > MAX_EDIT_BYTES
        || draft.contains(['\0', '\r'])
    {
        return Err(io::Error::other(
            "Unsupported text or draft exceeds 64 KiB.",
        ));
    }
    let path = path.canonicalize()?; // Replace the target, not the symlink.
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("File has no parent directory."))?;
    let current = read_regular(&path, MAX_EDIT_BYTES)?;
    if current != expected.as_bytes() {
        return Err(io::Error::other(
            "The file changed on disk. Copy your draft before reloading.",
        ));
    }
    editable(expected).map_err(io::Error::other)?;
    editable(draft).map_err(io::Error::other)?;
    let mut contents = String::new();
    if expected.starts_with('\u{feff}') {
        contents.push('\u{feff}');
    }
    if expected.contains("\r\n") {
        contents.push_str(&draft.replace('\n', "\r\n"));
    } else {
        contents.push_str(draft);
    }
    editable(&contents).map_err(io::Error::other)?;
    let permissions = fs::metadata(&path)?.permissions();
    if permissions.readonly() {
        return Err(io::Error::other("File is read-only."));
    }
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(contents.as_bytes())?;
    staged.as_file().set_permissions(permissions)?;
    staged.as_file().sync_all()?;
    if read_regular(&path, MAX_EDIT_BYTES)? != current {
        return Err(io::Error::other(
            "The file changed on disk. Copy your draft before reloading.",
        ));
    }
    staged.persist(&path).map_err(|err| err.error)?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_preserves_bom_crlf_and_checks_for_external_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("example.rs");
        let original = "\u{feff}hola\r\nmundo\r\n";
        fs::write(&path, original).unwrap();
        let saved = save(&path, original, "hola\n🌊").unwrap();
        assert_eq!(saved, "\u{feff}hola\r\n🌊");
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        assert!(save(&path, original, "stale").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), saved);
    }

    #[test]
    fn detects_indent_and_line_endings_for_the_footer() {
        assert_eq!(
            detect_indent("fn a() {\n    b();\n}\n"),
            Some(Indent::Spaces { size: 4 })
        );
        assert_eq!(
            detect_indent("a:\n  b: 1\n"),
            Some(Indent::Spaces { size: 2 })
        );
        assert_eq!(detect_indent("\tx\n\t\ty"), Some(Indent::Tabs));
        assert_eq!(detect_indent("plain\n"), None);
        assert_eq!(
            detect_indent("/**\n * doc\n */\nfn x() {\n  a\n}"),
            Some(Indent::Spaces { size: 2 })
        );
        assert!(text_info("a.rs", "a\r\nb\n").crlf);
        assert!(!text_info("a.rs", "a\nb\r\n").crlf);
        assert!(!text_info("a.rs", "a").crlf);
        let bom = text_info("Cargo.toml", "\u{feff}[package]\n");
        assert!(bom.bom);
        assert_eq!(bom.language, "TOML");
    }

    #[test]
    fn detects_binary_and_bounds_native_editor_work() {
        assert_eq!(text(b"a\0b"), None);
        assert_eq!(text(b"\xff"), None);
        assert!(editable("a\r\nb\n").is_err());
        assert!(editable(&"一".repeat(2048)).is_err());
        assert!(editable("a\r\nb\r\n").is_ok());
    }
}
