//! OS-specific helpers isolated behind a small platform boundary.

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[cfg(target_os = "linux")]
mod background_blur;
mod desktop_notifications;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as os;
#[cfg(windows)]
use windows as os;

pub use desktop_notifications::{DesktopNotification, DesktopNotifier};

pub fn has_wayland_surface(window: &impl raw_window_handle::HasWindowHandle) -> bool {
    window.window_handle().is_ok_and(|handle| {
        matches!(
            handle.as_raw(),
            raw_window_handle::RawWindowHandle::Wayland(_)
        )
    })
}

#[cfg(target_os = "linux")]
pub use background_blur::WindowBackgroundBlur;

#[cfg(not(target_os = "linux"))]
pub struct WindowBackgroundBlur;

#[cfg(not(target_os = "linux"))]
impl WindowBackgroundBlur {
    pub fn attach<W>(_window: std::sync::Arc<W>) -> Option<Self> {
        None
    }

    pub fn set_enabled(&self, _enabled: bool) {}
}

/// The platform's standard text-editing shortcut modifier, excluding AltGr.
pub fn editor_command_modifier(control: bool, alt: bool, super_key: bool) -> bool {
    if cfg!(target_os = "macos") {
        super_key
    } else {
        control && !alt
    }
}

/// How an external editor CLI accepts a file + optional line/column target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorLaunchStyle {
    /// VS Code family (`code`, `cursor`, `windsurf`, `antigravity`, `codium`, `trae`, `kiro`):
    /// uses `--goto <file>:<line>[:<col>]` for line jumps.
    VsCodeGoto,
    /// Editors accepting `<file>:<line>[:<col>]` directly (`zed`, `subl`).
    ColonLocation,
    /// JetBrains IDEs (`idea`, `webstorm`, `rustrover`, ...):
    /// uses `--line <line> [--column <col>] <file>`.
    JetBrainsLine,
    /// Fallback GUI / `open -a` launchers that only take the target path.
    PathOnly,
}

/// An installed external IDE / code editor detected on the host OS.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEditor {
    /// Stable identifier (also matches `editor:<id>` icons in `icy-app`).
    pub id: String,
    /// Human-readable display label (e.g. `"Cursor"`, `"VS Code"`, `"Zed"`).
    pub name: String,
    #[serde(skip)]
    pub command: PathBuf,
    #[serde(skip)]
    pub base_args: Vec<String>,
    #[serde(skip)]
    pub launch_style: EditorLaunchStyle,
}

/// Builds the full argument vector to pass to `editor.command` for `target`.
pub fn build_editor_args(
    editor: &ExternalEditor,
    target: &Path,
    is_dir: bool,
    line: Option<u32>,
    col: Option<u32>,
) -> Vec<String> {
    let mut args = editor.base_args.clone();
    let target_str = target.to_string_lossy().into_owned();
    let line = (!is_dir).then_some(line).flatten().filter(|&l| l > 0);
    let col = line.and(col).filter(|&c| c > 0);

    match (editor.launch_style, line) {
        (EditorLaunchStyle::VsCodeGoto, Some(l)) => {
            args.push("--goto".into());
            args.push(match col {
                Some(c) => format!("{target_str}:{l}:{c}"),
                None => format!("{target_str}:{l}"),
            });
        }
        (EditorLaunchStyle::ColonLocation, Some(l)) => {
            args.push(match col {
                Some(c) => format!("{target_str}:{l}:{c}"),
                None => format!("{target_str}:{l}"),
            });
        }
        (EditorLaunchStyle::JetBrainsLine, Some(l)) => {
            args.push("--line".into());
            args.push(l.to_string());
            if let Some(c) = col {
                args.push("--column".into());
                args.push(c.to_string());
            }
            args.push(target_str);
        }
        _ => args.push(target_str),
    }
    args
}

/// Parses a freedesktop `.desktop` `Exec=` value into `(program, base_args)`,
/// removing desktop field codes (`%F`, `%U`, `%f`, `%u`, etc.).
#[cfg(any(unix, test))]
pub(crate) fn parse_desktop_exec(exec_line: &str) -> Option<(String, Vec<String>)> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in exec_line.trim().chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let mut iter = tokens.into_iter().filter(|tok| {
        !(tok.starts_with('%') && tok.len() == 2 && tok.as_bytes()[1].is_ascii_alphabetic())
    });
    let first = iter.next()?;
    if first == "env" {
        let rest: Vec<String> = iter.skip_while(|t| t.contains('=')).collect();
        let (cmd, args) = rest.split_first()?;
        return Some((cmd.clone(), args.to_vec()));
    }
    Some((first, iter.collect()))
}

/// Returns the user's preferred interactive shell program and default flags.
pub fn default_shell() -> (String, Vec<String>) {
    os::default_shell()
}

/// Returns a cross-platform shell invocation that prints `message` and exits.
pub fn echo_command(message: &str) -> (String, Vec<String>) {
    os::echo_command(message)
}

/// Returns the user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    os::home_dir()
}

/// Named folders: `(kind or label, path)`.
pub type Places = Vec<(String, PathBuf)>;

/// File-manager "places" (Nautilus sidebar): XDG user dirs as `(kind, path)`, e.g.
/// `("download", ~/Descargas)`, and GTK bookmarks as `(label, path)`. Missing folders are dropped.
pub fn user_places() -> (Places, Places) {
    let (special, bookmarks) = os::user_places();
    let exists = |list: Places| list.into_iter().filter(|(_, path)| path.is_dir()).collect();
    (exists(special), exists(bookmarks))
}

/// Parses `user-dirs.dirs` (`XDG_DOWNLOAD_DIR="$HOME/Descargas"`) into `(kind, path)` pairs.
/// XDG disables a directory by pointing it at `$HOME`, so those are skipped.
pub fn parse_user_dirs(text: &str, home: &Path) -> Vec<(String, PathBuf)> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            let kind = key
                .strip_prefix("XDG_")?
                .strip_suffix("_DIR")?
                .to_ascii_lowercase();
            let value = value.trim().trim_matches('"');
            let path = match value.strip_prefix("$HOME") {
                Some(rest) => home.join(rest.trim_start_matches('/')),
                None if value.starts_with('/') => PathBuf::from(value),
                None => return None,
            };
            (path != home).then_some((kind, path))
        })
        .collect()
}

/// Parses GTK's `bookmarks` file (`file:///path/with%20space Optional label`) into
/// `(label, path)` pairs; non-local URIs (sftp://, smb://) are skipped.
pub fn parse_gtk_bookmarks(text: &str) -> Vec<(String, PathBuf)> {
    text.lines()
        .filter_map(|line| {
            let (uri, label) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
            let path = PathBuf::from(crate::links::percent_decode(uri.strip_prefix("file://")?));
            let label = match label.trim() {
                "" => path.file_name()?.to_str()?.to_string(),
                label => label.to_string(),
            };
            Some((label, path))
        })
        .collect()
}

/// Returns icy's persistent configuration directory (`~/.config/icy` or `%APPDATA%\icy`).
pub fn config_dir() -> PathBuf {
    os::config_dir()
}

pub(crate) use os::{
    connect_session_socket, listen_session_socket, prepare_session_dir, session_server_command,
};

/// Command line (NUL-separated argv) of the PTY's foreground job, used to recognise which agent
/// CLI is running inside a plain shell session. `None` where the OS offers no cheap lookup.
pub fn foreground_cmdline(master: &(dyn portable_pty::MasterPty + Send)) -> Option<String> {
    os::foreground_cmdline(master)
}

/// Opens a URL or file in the host operating system's default handler.
pub fn open_external(target: &str) -> io::Result<()> {
    os::open_external(target)
}

/// Detects installed external IDEs and code editors on the host OS.
pub fn detect_editors() -> Vec<ExternalEditor> {
    os::detect_editors()
}

/// Opens a file (with optional 1-based line/column) or directory in a detected external editor.
pub fn open_in_editor(
    editor: &ExternalEditor,
    target: &Path,
    line: Option<u32>,
    col: Option<u32>,
) -> io::Result<()> {
    os::open_in_editor(editor, target, line, col)
}

/// Opens a directory or reveals a file in the OS file manager.
pub fn reveal_in_file_manager(target: &Path) -> io::Result<()> {
    os::reveal_in_file_manager(target)
}

/// Expected GitHub release artifact stem for the current platform.
pub fn release_asset_name() -> &'static str {
    os::release_asset_name()
}

/// Stages an OS-native helper that replaces `current_exe` with `staged_bin` and restarts `icy`.
pub fn spawn_self_update_installer(staged_bin: &Path, current_exe: &Path) -> io::Result<()> {
    os::spawn_self_update_installer(staged_bin, current_exe)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_xdg_user_dirs_and_gtk_bookmarks() {
        let home = Path::new("/home/ada");
        let dirs = "# comment\nXDG_DOWNLOAD_DIR=\"$HOME/Descargas\"\nXDG_PUBLICSHARE_DIR=\"$HOME/\"\nXDG_MUSIC_DIR=\"/mnt/music\"\n";
        assert_eq!(
            parse_user_dirs(dirs, home),
            vec![
                ("download".to_string(), PathBuf::from("/home/ada/Descargas")),
                ("music".to_string(), PathBuf::from("/mnt/music")),
            ]
        );
        let marks = "file:///home/ada/my%20code Code\nfile:///srv/data\nsftp://host/x Remote\n";
        assert_eq!(
            parse_gtk_bookmarks(marks),
            vec![
                ("Code".to_string(), PathBuf::from("/home/ada/my code")),
                ("data".to_string(), PathBuf::from("/srv/data")),
            ]
        );
    }

    use super::*;

    fn editor(style: EditorLaunchStyle, base_args: &[&str]) -> ExternalEditor {
        ExternalEditor {
            id: "test".into(),
            name: "Test".into(),
            command: PathBuf::from("test"),
            base_args: base_args.iter().map(|s| (*s).into()).collect(),
            launch_style: style,
        }
    }

    #[test]
    fn builds_editor_args_per_launch_style() {
        let file = Path::new("/tmp/my project/main.rs");
        let dir = Path::new("/tmp/my project");

        assert_eq!(
            build_editor_args(
                &editor(EditorLaunchStyle::VsCodeGoto, &[]),
                file,
                false,
                Some(42),
                Some(7),
            ),
            vec!["--goto", "/tmp/my project/main.rs:42:7"]
        );
        assert_eq!(
            build_editor_args(
                &editor(EditorLaunchStyle::VsCodeGoto, &[]),
                dir,
                true,
                Some(42),
                Some(7),
            ),
            vec!["/tmp/my project"]
        );
        assert_eq!(
            build_editor_args(
                &editor(EditorLaunchStyle::ColonLocation, &[]),
                file,
                false,
                Some(12),
                None,
            ),
            vec!["/tmp/my project/main.rs:12"]
        );
        assert_eq!(
            build_editor_args(
                &editor(EditorLaunchStyle::JetBrainsLine, &[]),
                file,
                false,
                Some(9),
                Some(3),
            ),
            vec!["--line", "9", "--column", "3", "/tmp/my project/main.rs"]
        );
        assert_eq!(
            build_editor_args(
                &editor(EditorLaunchStyle::PathOnly, &["-a", "Cursor"]),
                file,
                false,
                Some(9),
                None,
            ),
            vec!["-a", "Cursor", "/tmp/my project/main.rs"]
        );
    }

    #[test]
    fn parses_freedesktop_exec_lines() {
        assert_eq!(
            parse_desktop_exec(r#""/usr/share/cursor/cursor" --unity-launch %F"#),
            Some((
                "/usr/share/cursor/cursor".into(),
                vec!["--unity-launch".into()]
            ))
        );
        assert_eq!(
            parse_desktop_exec("flatpak run --branch=stable com.visualstudio.code %F"),
            Some((
                "flatpak".into(),
                vec![
                    "run".into(),
                    "--branch=stable".into(),
                    "com.visualstudio.code".into()
                ]
            ))
        );
        assert_eq!(
            parse_desktop_exec("env BAMF_DESKTOP_FILE_HINT=foo /opt/zed/zed %U"),
            Some(("/opt/zed/zed".into(), Vec::new()))
        );
    }
}
