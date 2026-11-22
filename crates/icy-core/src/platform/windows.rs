use std::env;

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

fn find_in_path(executable: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| dir.join(executable).is_file())
}
