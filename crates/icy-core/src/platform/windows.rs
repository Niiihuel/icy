use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn default_shell() -> (String, Vec<String>) {
    if find_in_path("pwsh.exe") {
        return ("pwsh.exe".to_string(), vec!["-NoLogo".to_string()]);
    }
    let comspec = env::var("COMSPEC")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "cmd.exe".to_string());
    (comspec, Vec::new())
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

pub(super) fn config_dir() -> PathBuf {
    if let Some(appdata) = env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        return PathBuf::from(appdata).join("icy");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("icy")
}

pub(super) fn open_external(target: &str) -> io::Result<()> {
    Command::new("cmd.exe")
        .args(["/C", "start", "", target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
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
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn find_in_path(executable: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| dir.join(executable).is_file())
}
