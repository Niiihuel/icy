use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

pub(super) fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub(super) fn config_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("icy");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/icy")
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
