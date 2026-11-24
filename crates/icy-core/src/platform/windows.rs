use std::env;
use std::fs;
use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{EditorLaunchStyle, ExternalEditor, build_editor_args};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub(super) fn default_shell() -> (String, Vec<String>) {
    if find_in_path("pwsh.exe").is_some() {
        return ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]);
    }
    let comspec = env::var("COMSPEC")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "cmd.exe".to_string());
    (comspec, Vec::new())
}

pub(super) fn foreground_cmdline(_master: &(dyn portable_pty::MasterPty + Send)) -> Option<String> {
    // ConPTY has no foreground-process query; sessions fall back to OSC-title inference.
    None
}

pub(super) fn echo_command(message: &str) -> (String, Vec<String>) {
    let comspec = env::var("COMSPEC")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "cmd.exe".to_string());
    (comspec, vec!["/C".to_string(), format!("echo {message}")])
}

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

pub(super) fn user_places() -> (super::Places, super::Places) {
    // shortcut: default Known Folder names under the profile; redirected folders (OneDrive) need SHGetKnownFolderPath.
    let Some(home) = home_dir() else {
        return Default::default();
    };
    let special = [
        ("desktop", "Desktop"),
        ("documents", "Documents"),
        ("download", "Downloads"),
        ("music", "Music"),
        ("pictures", "Pictures"),
        ("videos", "Videos"),
    ]
    .into_iter()
    .map(|(kind, name)| (kind.to_string(), home.join(name)))
    .collect();
    (special, Vec::new())
}

pub(super) fn config_dir() -> PathBuf {
    if let Some(appdata) = env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(appdata).join("icy");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("icy")
}

pub(crate) fn prepare_session_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

fn session_pipe_name(path: &Path) -> io::Result<String> {
    use std::os::windows::ffi::OsStrExt;
    let directory = path.parent().unwrap_or(path).canonicalize()?;
    // Stable across Rust versions so a new executable can find the existing server.
    let hash = directory
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        });
    Ok(format!("icy-session-{hash:016x}"))
}

pub(crate) fn connect_session_socket(
    path: &Path,
) -> io::Result<interprocess::local_socket::Stream> {
    use interprocess::local_socket::{
        ConnectOptions, GenericNamespaced, ToNsName, traits::StreamCommon as _,
    };
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let stream = ConnectOptions::new()
        .name(session_pipe_name(path)?.to_ns_name::<GenericNamespaced>()?)
        .wait_mode(interprocess::ConnectWaitMode::Deferred)
        .connect_sync()?;
    let pid = stream
        .peer_creds()?
        .pid()
        .ok_or_else(|| io::Error::other("unknown session server identity"))?;
    // SAFETY: Windows supplies the peer PID. The successfully opened process handle is owned below.
    let peer = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if peer.is_null() {
        return Err(io::Error::last_os_error());
    }
    let peer = unsafe { OwnedHandle::from_raw_handle(peer) };
    if process_account_sid(peer.as_raw_handle())?
        != process_account_sid(unsafe { GetCurrentProcess() })?
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "session server is owned by a different account",
        ));
    }
    Ok(stream)
}

pub(crate) fn listen_session_socket(
    path: &Path,
) -> io::Result<interprocess::local_socket::Listener> {
    use interprocess::local_socket::{GenericNamespaced, ListenerOptions, ToNsName};
    use interprocess::os::windows::local_socket::ListenerOptionsExt;
    ListenerOptions::new()
        .name(session_pipe_name(path)?.to_ns_name::<GenericNamespaced>()?)
        .security_descriptor(session_pipe_security()?)
        .create_sync()
}

fn session_pipe_security()
-> io::Result<interprocess::os::windows::security_descriptor::SecurityDescriptor> {
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    use widestring::U16CString;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let account = process_account_sid(unsafe { GetCurrentProcess() })?;
    let sddl = U16CString::from_str(format!("D:P(A;;GA;;;SY)(A;;GRGW;;;{account})"))
        .map_err(io::Error::other)?;
    SecurityDescriptor::deserialize(&sddl)
}

fn process_account_sid(process: windows_sys::Win32::Foundation::HANDLE) -> io::Result<String> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::null_mut;
    use widestring::U16CStr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
        TokenUser,
    };
    use windows_sys::Win32::System::Threading::OpenProcessToken;

    let mut token = null_mut();
    // SAFETY: each Win32 buffer below is aligned, writable and kept alive until its handle or SID is consumed.
    unsafe {
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle::from_raw_handle(token);
        let mut size = 0;
        GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut size);
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut sid = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err(io::Error::last_os_error());
        }
        let account = U16CStr::from_ptr_str(sid).to_string_lossy();
        LocalFree(sid.cast());
        Ok(account)
    }
}

pub(crate) fn session_server_command(executable: &Path, dir: &Path) -> io::Result<Command> {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    // A running image is locked on Windows. The server uses its own copy so an app update
    // can replace icy.exe while the PTYs stay attached to the server.
    let server = dir.join("icy-session-server.exe");
    fs::copy(executable, &server)?;
    let executable_dir = executable
        .parent()
        .ok_or_else(|| io::Error::other("missing executable directory"))?;
    let path = env::var_os("PATH").unwrap_or_default();
    let path = env::join_paths(
        std::iter::once(executable_dir.to_path_buf()).chain(env::split_paths(&path)),
    )
    .map_err(io::Error::other)?;
    let mut command = Command::new(server);
    command
        .creation_flags(DETACHED_PROCESS)
        .current_dir(executable_dir)
        .env("PATH", path);
    Ok(command)
}

pub(super) fn open_external(target: &str) -> io::Result<()> {
    Command::new("cmd.exe")
        .args(["/C", "start", "", target])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

pub(super) fn reveal_in_file_manager(target: &Path) -> io::Result<()> {
    let mut cmd = Command::new("explorer.exe");
    if target.is_dir() {
        cmd.arg(target);
    } else {
        let clean = target.display().to_string().replace('"', "");
        cmd.raw_arg(format!("/select,\"{clean}\""));
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
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
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

struct WinEditorSpec {
    id: &'static str,
    name: &'static str,
    style: EditorLaunchStyle,
    binaries: &'static [&'static str],
    local_app_data: &'static [&'static str],
    program_files: &'static [&'static str],
}

const WIN_EDITORS: &[WinEditorSpec] = &[
    WinEditorSpec {
        id: "cursor",
        name: "Cursor",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["cursor.cmd", "cursor.exe"],
        local_app_data: &[
            r"Programs\cursor\resources\app\bin\cursor.cmd",
            r"Programs\cursor\Cursor.exe",
        ],
        program_files: &[r"cursor\resources\app\bin\cursor.cmd"],
    },
    WinEditorSpec {
        id: "vscode",
        name: "VS Code",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["code.cmd", "code.exe"],
        local_app_data: &[r"Programs\Microsoft VS Code\bin\code.cmd"],
        program_files: &[r"Microsoft VS Code\bin\code.cmd"],
    },
    WinEditorSpec {
        id: "vscode-insiders",
        name: "VS Code Insiders",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["code-insiders.cmd", "code-insiders.exe"],
        local_app_data: &[r"Programs\Microsoft VS Code Insiders\bin\code-insiders.cmd"],
        program_files: &[r"Microsoft VS Code Insiders\bin\code-insiders.cmd"],
    },
    WinEditorSpec {
        id: "vscodium",
        name: "VSCodium",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["codium.cmd", "codium.exe"],
        local_app_data: &[r"Programs\VSCodium\bin\codium.cmd"],
        program_files: &[r"VSCodium\bin\codium.cmd"],
    },
    WinEditorSpec {
        id: "windsurf",
        name: "Windsurf",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["windsurf.cmd", "windsurf.exe"],
        local_app_data: &[
            r"Programs\Windsurf\bin\windsurf.cmd",
            r"Programs\Windsurf\Windsurf.exe",
        ],
        program_files: &[r"Windsurf\bin\windsurf.cmd"],
    },
    WinEditorSpec {
        id: "antigravity",
        name: "Antigravity",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["antigravity.cmd", "antigravity.exe"],
        local_app_data: &[
            r"Programs\Antigravity\bin\antigravity.cmd",
            r"Programs\Antigravity\Antigravity.exe",
        ],
        program_files: &[r"Antigravity\bin\antigravity.cmd"],
    },
    WinEditorSpec {
        id: "zed",
        name: "Zed",
        style: EditorLaunchStyle::ColonLocation,
        binaries: &["zed.exe", "zed.cmd"],
        local_app_data: &[r"Programs\Zed\Zed.exe"],
        program_files: &[r"Zed\Zed.exe"],
    },
    WinEditorSpec {
        id: "trae",
        name: "Trae",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["trae.cmd", "trae.exe"],
        local_app_data: &[r"Programs\Trae\bin\trae.cmd"],
        program_files: &[r"Trae\bin\trae.cmd"],
    },
    WinEditorSpec {
        id: "kiro",
        name: "Kiro",
        style: EditorLaunchStyle::VsCodeGoto,
        binaries: &["kiro.cmd", "kiro.exe"],
        local_app_data: &[r"Programs\Kiro\bin\kiro.cmd"],
        program_files: &[r"Kiro\bin\kiro.cmd"],
    },
    WinEditorSpec {
        id: "sublime",
        name: "Sublime Text",
        style: EditorLaunchStyle::ColonLocation,
        binaries: &["subl.exe"],
        local_app_data: &[],
        program_files: &[r"Sublime Text\subl.exe", r"Sublime Text 3\subl.exe"],
    },
    WinEditorSpec {
        id: "rustrover",
        name: "RustRover",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["rustrover.cmd", "rustrover64.exe", "rustrover.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\rustrover.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "idea",
        name: "IntelliJ IDEA",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["idea.cmd", "idea64.exe", "idea.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\idea.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "webstorm",
        name: "WebStorm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["webstorm.cmd", "webstorm64.exe", "webstorm.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\webstorm.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "pycharm",
        name: "PyCharm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["pycharm.cmd", "pycharm64.exe", "pycharm.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\pycharm.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "goland",
        name: "GoLand",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["goland.cmd", "goland64.exe", "goland.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\goland.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "clion",
        name: "CLion",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["clion.cmd", "clion64.exe", "clion.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\clion.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "phpstorm",
        name: "PhpStorm",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["phpstorm.cmd", "phpstorm64.exe", "phpstorm.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\phpstorm.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "rider",
        name: "Rider",
        style: EditorLaunchStyle::JetBrainsLine,
        binaries: &["rider.cmd", "rider64.exe", "rider.exe"],
        local_app_data: &[r"JetBrains\Toolbox\scripts\rider.cmd"],
        program_files: &[],
    },
    WinEditorSpec {
        id: "neovide",
        name: "Neovide",
        style: EditorLaunchStyle::PathOnly,
        binaries: &["neovide.exe"],
        local_app_data: &[],
        program_files: &[r"Neovide\neovide.exe"],
    },
];

pub(super) fn detect_editors() -> Vec<ExternalEditor> {
    let local = env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let pf_dirs: Vec<PathBuf> = [
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .collect();

    let mut detected = Vec::new();
    for spec in WIN_EDITORS {
        let found = spec
            .binaries
            .iter()
            .find_map(|bin| find_in_path(bin))
            .or_else(|| {
                let root = local.as_ref()?;
                spec.local_app_data
                    .iter()
                    .map(|rel| root.join(rel))
                    .find(|p| p.is_file())
            })
            .or_else(|| {
                pf_dirs.iter().find_map(|root| {
                    spec.program_files
                        .iter()
                        .map(|rel| root.join(rel))
                        .find(|p| p.is_file())
                })
            });

        if let Some(command) = found {
            detected.push(ExternalEditor {
                id: spec.id.into(),
                name: spec.name.into(),
                command,
                base_args: Vec::new(),
                launch_style: spec.style,
            });
        }
    }
    detected
}

pub(super) fn release_asset_name() -> &'static str {
    "icy-windows-x86_64"
}

pub(super) fn spawn_self_update_installer(staged_bin: &Path, current_exe: &Path) -> io::Result<()> {
    let pid = std::process::id();
    let script_path = env::temp_dir().join(format!("icy-apply-update-{pid}.cmd"));
    let staged = staged_bin.display();
    let target = current_exe.display();
    let script = format!(
        "@echo off\r\n\
         ping 127.0.0.1 -n 2 >nul\r\n\
         move /Y \"{staged}\" \"{target}\" >nul\r\n\
         start \"\" \"{target}\"\r\n\
         del \"%~f0\" >nul 2>&1\r\n"
    );
    fs::write(&script_path, script)?;
    Command::new("cmd.exe")
        .args(["/C", &script_path.to_string_lossy()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn find_in_path(executable: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.is_file())
}
