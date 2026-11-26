use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(not(target_os = "macos"))]
use super::parse_desktop_exec;
use super::{EditorLaunchStyle, ExternalEditor, build_editor_args};

pub(super) fn default_shell() -> (String, Vec<String>) {
    let shell = env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "/bin/sh".to_string());
    (shell, Vec::new())
}

pub(super) fn echo_command(message: &str) -> (String, Vec<String>) {
    (
        "/bin/sh".to_string(),
        vec!["-c".to_string(), format!("printf '%s\\n' '{message}'")],
    )
}

pub(super) fn foreground_cmdline(master: &(dyn portable_pty::MasterPty + Send)) -> Option<String> {
    let pgid = master.process_group_leader()?;
    // shortcut: /proc is Linux-only; macOS returns None and falls back to OSC-title inference.
    let raw = fs::read(format!("/proc/{pgid}/cmdline")).ok()?;
    Some(String::from_utf8_lossy(&raw).into_owned())
}

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub(super) fn user_places() -> (super::Places, super::Places) {
    let Some(home) = home_dir() else {
        return Default::default();
    };
    let config = env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let read = |path: PathBuf| fs::read_to_string(path).unwrap_or_default();
    (
        super::parse_user_dirs(&read(config.join("user-dirs.dirs")), &home),
        super::parse_gtk_bookmarks(&read(config.join("gtk-3.0/bookmarks"))),
    )
}

pub(super) fn config_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("icy");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/icy")
}

pub(crate) fn prepare_session_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: geteuid has no pointer arguments and reads only the process credentials.
    if !metadata.is_dir() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid session server directory",
        ));
    }
    if metadata.mode() & 0o077 != 0 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(crate) fn connect_session_socket(
    path: &Path,
) -> io::Result<interprocess::local_socket::Stream> {
    use interprocess::local_socket::{GenericFilePath, ToFsName, traits::Stream as _};
    interprocess::local_socket::Stream::connect(path.to_fs_name::<GenericFilePath>()?)
}

pub(crate) fn listen_session_socket(
    path: &Path,
) -> io::Result<interprocess::local_socket::Listener> {
    use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName};
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    ListenerOptions::new()
        .name(path.to_fs_name::<GenericFilePath>()?)
        .create_sync()
}

pub(crate) fn session_server_command(executable: &Path, _dir: &Path) -> io::Result<Command> {
    let mut command = Command::new(executable);
    // SAFETY: the child only calls the async-signal-safe setsid before exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command)
}

pub(super) fn open_external(target: &str) -> io::Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub(super) fn reveal_in_file_manager(target: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        if !target.is_dir() {
            cmd.arg("-R");
        }
        cmd.arg(target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let dir = if target.is_dir() {
            target
        } else {
            target.parent().unwrap_or(target)
        };
        Command::new("xdg-open")
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }
}

pub(super) fn open_in_editor(
    editor: &ExternalEditor,
    target: &Path,
    line: Option<u32>,
    col: Option<u32>,
) -> io::Result<()> {
    let args = build_editor_args(editor, target, target.is_dir(), line, col);
    Command::new(&editor.command)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

struct UnixEditorSpec {
    id: &'static str,
    name: &'static str,
    style: EditorLaunchStyle,
    binaries: &'static [&'static str],
    #[cfg(target_os = "macos")]
    mac_apps: &'static [(&'static str, Option<&'static str>)],
    #[cfg(not(target_os = "macos"))]
    desktop_ids: &'static [&'static str],
    #[cfg(not(target_os = "macos"))]
    extra_bins: &'static [&'static str],
}

const UNIX_EDITORS: &[UnixEditorSpec] = &[
    UnixEditorSpec {
        id: "cursor",
        name: "Cursor",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["cursor"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Cursor.app", Some("Contents/Resources/app/bin/cursor"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["cursor.desktop", "cursor-cursor.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/cursor/cursor", "/opt/cursor/cursor"],
    },
    UnixEditorSpec {
        id: "vscode",
        name: "VS Code",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["code"],
        #[cfg(target_os = "macos")]
        mac_apps: &[(
            "Visual Studio Code.app",
            Some("Contents/Resources/app/bin/code"),
        )],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &[
            "code.desktop",
            "com.visualstudio.code.desktop",
            "code_code.desktop",
        ],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/code/bin/code"],
    },
    UnixEditorSpec {
        id: "vscode-insiders",
        name: "VS Code Insiders",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["code-insiders"],
        #[cfg(target_os = "macos")]
        mac_apps: &[(
            "Visual Studio Code - Insiders.app",
            Some("Contents/Resources/app/bin/code"),
        )],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["code-insiders.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/code-insiders/bin/code-insiders"],
    },
    UnixEditorSpec {
        id: "vscodium",
        name: "VSCodium",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["codium"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("VSCodium.app", Some("Contents/Resources/app/bin/codium"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["codium.desktop", "com.vscodium.codium.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/codium/bin/codium"],
    },
    UnixEditorSpec {
        id: "windsurf",
        name: "Windsurf",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["windsurf"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Windsurf.app", Some("Contents/Resources/app/bin/windsurf"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["windsurf.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/windsurf/bin/windsurf"],
    },
    UnixEditorSpec {
        id: "antigravity",
        name: "Antigravity",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["antigravity"],
        #[cfg(target_os = "macos")]
        mac_apps: &[(
            "Antigravity.app",
            Some("Contents/Resources/app/bin/antigravity"),
        )],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["antigravity.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/usr/share/antigravity/bin/antigravity"],
    },
    UnixEditorSpec {
        id: "zed",
        name: "Zed",
        style: EditorLaunchStyle::ColonLocation,
        binaries: &["zed", "zeditor"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Zed.app", Some("Contents/MacOS/cli"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["dev.zed.Zed.desktop", "zed.desktop", "zeditor.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "trae",
        name: "Trae",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["trae"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Trae.app", Some("Contents/Resources/app/bin/trae"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["trae.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "kiro",
        name: "Kiro",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["kiro"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Kiro.app", Some("Contents/Resources/app/bin/kiro"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["kiro.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "sublime",
        name: "Sublime Text",
        style: EditorLaunchStyle::ColonLocation,
        binaries: &["subl"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Sublime Text.app", Some("Contents/SharedSupport/bin/subl"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["sublime_text.desktop", "com.sublimetext.four.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &["/opt/sublime_text/sublime_text"],
    },
    UnixEditorSpec {
        id: "rustrover",
        name: "RustRover",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["rustrover"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("RustRover.app", Some("Contents/MacOS/rustrover"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-rustrover.desktop", "rustrover.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "idea",
        name: "IntelliJ IDEA",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["idea", "intellij-idea-ultimate", "intellij-idea-community"],
        #[cfg(target_os = "macos")]
        mac_apps: &[
            ("IntelliJ IDEA.app", Some("Contents/MacOS/idea")),
            ("IntelliJ IDEA CE.app", Some("Contents/MacOS/idea")),
        ],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &[
            "jetbrains-idea.desktop",
            "jetbrains-idea-ce.desktop",
            "intellij-idea-ultimate.desktop",
            "intellij-idea-community.desktop",
        ],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "webstorm",
        name: "WebStorm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["webstorm"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("WebStorm.app", Some("Contents/MacOS/webstorm"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-webstorm.desktop", "webstorm.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "pycharm",
        name: "PyCharm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["pycharm", "pycharm-professional", "pycharm-community"],
        #[cfg(target_os = "macos")]
        mac_apps: &[
            ("PyCharm.app", Some("Contents/MacOS/pycharm")),
            ("PyCharm CE.app", Some("Contents/MacOS/pycharm")),
        ],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &[
            "jetbrains-pycharm.desktop",
            "jetbrains-pycharm-ce.desktop",
            "pycharm.desktop",
        ],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "goland",
        name: "GoLand",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["goland"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("GoLand.app", Some("Contents/MacOS/goland"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-goland.desktop", "goland.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "clion",
        name: "CLion",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["clion"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("CLion.app", Some("Contents/MacOS/clion"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-clion.desktop", "clion.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "phpstorm",
        name: "PhpStorm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["phpstorm"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("PhpStorm.app", Some("Contents/MacOS/phpstorm"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-phpstorm.desktop", "phpstorm.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "rider",
        name: "Rider",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["rider"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Rider.app", Some("Contents/MacOS/rider"))],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["jetbrains-rider.desktop", "rider.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
    UnixEditorSpec {
        id: "neovide",
        name: "Neovide",
        style: EditorLaunchStyle::PathOnly,
        binaries: &["neovide"],
        #[cfg(target_os = "macos")]
        mac_apps: &[("Neovide.app", None)],
        #[cfg(not(target_os = "macos"))]
        desktop_ids: &["neovide.desktop"],
        #[cfg(not(target_os = "macos"))]
        extra_bins: &[],
    },
];

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && (meta.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

fn find_in_dirs(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn executable_search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|raw| env::split_paths(&raw).collect())
        .unwrap_or_default();
    if let Some(home) = home_dir() {
        #[cfg(target_os = "macos")]
        dirs.push(home.join("Library/Application Support/JetBrains/Toolbox/scripts"));
        #[cfg(not(target_os = "macos"))]
        {
            dirs.push(home.join(".local/bin"));
            dirs.push(home.join(".local/share/JetBrains/Toolbox/scripts"));
        }
    }
    dirs
}

#[cfg(not(target_os = "macos"))]
fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local/share/applications"));
        dirs.push(home.join(".local/share/flatpak/exports/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));
    dirs
}

#[cfg(not(target_os = "macos"))]
fn resolve_from_desktop(
    dirs: &[PathBuf],
    desktop_ids: &[&str],
    search_dirs: &[PathBuf],
) -> Option<(PathBuf, Vec<String>)> {
    for dir in dirs {
        for desktop_id in desktop_ids {
            let path = dir.join(desktop_id);
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let mut in_entry = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_entry = trimmed == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(exec_raw) = trimmed.strip_prefix("Exec=")
                    && let Some((program, args)) = parse_desktop_exec(exec_raw)
                {
                    let candidate = PathBuf::from(&program);
                    if candidate.is_absolute() && is_executable(&candidate) {
                        return Some((candidate, args));
                    }
                    if let Some(found) = find_in_dirs(search_dirs, &program) {
                        return Some((found, args));
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn resolve_mac_app(spec: &UnixEditorSpec) -> Option<(PathBuf, Vec<String>, EditorLaunchStyle)> {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = home_dir() {
        roots.push(home.join("Applications"));
    }
    for root in roots {
        for &(app_name, cli_rel) in spec.mac_apps {
            let bundle = root.join(app_name);
            if !bundle.is_dir() {
                continue;
            }
            if let Some(rel) = cli_rel {
                let cli = bundle.join(rel);
                if is_executable(&cli) {
                    return Some((cli, Vec::new(), spec.style));
                }
            }
            return Some((
                PathBuf::from("open"),
                vec!["-a".into(), bundle.display().to_string()],
                EditorLaunchStyle::PathOnly,
            ));
        }
    }
    None
}

pub(super) fn detect_editors() -> Vec<ExternalEditor> {
    let search_dirs = executable_search_dirs();
    #[cfg(not(target_os = "macos"))]
    let app_desktop_dirs = desktop_dirs();
    let mut detected = Vec::new();

    for spec in UNIX_EDITORS {
        if let Some(cmd) = spec
            .binaries
            .iter()
            .find_map(|bin| find_in_dirs(&search_dirs, bin))
        {
            detected.push(ExternalEditor {
                id: spec.id.into(),
                name: spec.name.into(),
                command: cmd,
                base_args: Vec::new(),
                launch_style: spec.style,
            });
            continue;
        }

        #[cfg(target_os = "macos")]
        if let Some((command, base_args, launch_style)) = resolve_mac_app(spec) {
            detected.push(ExternalEditor {
                id: spec.id.into(),
                name: spec.name.into(),
                command,
                base_args,
                launch_style,
            });
            continue;
        }

        #[cfg(not(target_os = "macos"))]
        {
            if let Some(cmd) = spec
                .extra_bins
                .iter()
                .map(PathBuf::from)
                .find(|p| is_executable(p))
            {
                detected.push(ExternalEditor {
                    id: spec.id.into(),
                    name: spec.name.into(),
                    command: cmd,
                    base_args: Vec::new(),
                    launch_style: spec.style,
                });
                continue;
            }
            if let Some((command, base_args)) =
                resolve_from_desktop(&app_desktop_dirs, spec.desktop_ids, &search_dirs)
            {
                detected.push(ExternalEditor {
                    id: spec.id.into(),
                    name: spec.name.into(),
                    command,
                    base_args,
                    launch_style: spec.style,
                });
            }
        }
    }

    detected
}

pub(super) fn release_asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "icy-macos"
    } else {
        "icy-linux-x86_64"
    }
}

pub(super) fn spawn_self_update_installer(staged_bin: &Path, current_exe: &Path) -> io::Result<()> {
    let pid = std::process::id();
    let script_path = env::temp_dir().join(format!("icy-apply-update-{pid}.sh"));
    let staged = staged_bin.display();
    let target = current_exe.display();
    let script = format!(
        "#!/bin/sh\n\
         set -e\n\
         while kill -0 {pid} 2>/dev/null; do sleep 0.1; done\n\
         chmod +x \"{staged}\"\n\
         mv -f \"{staged}\" \"{target}\"\n\
         nohup \"{target}\" >/dev/null 2>&1 &\n\
         rm -f \"$0\"\n"
    );
    fs::write(&script_path, script)?;
    let mut perms = fs::metadata(&script_path)?.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&script_path, perms)?;

    Command::new("/bin/sh")
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
