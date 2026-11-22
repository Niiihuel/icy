use std::env;

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
