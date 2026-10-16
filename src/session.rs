use std::path::{Path, PathBuf};
use std::fs;

const DEFAULT_SOCKET_DIR: &str = "/tmp";

/// Returns the base directory for session files.
pub fn session_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("S_SESSION_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(DEFAULT_SOCKET_DIR)
}

/// Validate and sanitize session name to prevent path traversal.
pub fn sanitize_session_name(name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("Session name cannot be empty".to_string());
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err("Session name can only contain letters, numbers, hyphens, and underscores".to_string());
    }
    Ok(name.to_string())
}

/// Path to Unix Domain Socket: `/tmp/s_<name>.sock`
pub fn socket_path(name: &str) -> PathBuf {
    session_dir().join(format!("s_{}.sock", name))
}

/// Path to Log File: `/tmp/s_<name>.log`
pub fn log_path(name: &str) -> PathBuf {
    session_dir().join(format!("s_{}.log", name))
}

/// Path to PID File: `/tmp/s_<name>.pid`
pub fn pid_path(name: &str) -> PathBuf {
    session_dir().join(format!("s_{}.pid", name))
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub name: String,
    pub socket_path: PathBuf,
    pub log_path: PathBuf,
    pub is_alive: bool,
    pub log_size_bytes: u64,
    pub pid: Option<u32>,
}

#[cfg(unix)]
pub fn is_socket_alive(path: &Path) -> bool {
    use std::os::unix::net::UnixStream;
    if !path.exists() {
        return false;
    }
    match UnixStream::connect(path) {
        Ok(_) => true,
        Err(e) => {
            // If connection refused, socket file exists but nobody is listening
            if e.kind() == std::io::ErrorKind::ConnectionRefused {
                false
            } else {
                false
            }
        }
    }
}

#[cfg(not(unix))]
pub fn is_socket_alive(_path: &Path) -> bool {
    false
}

/// List all sessions found in the session directory.
pub fn list_sessions() -> std::io::Result<Vec<SessionInfo>> {
    let dir = session_dir();
    let mut sessions = Vec::new();

    if !dir.exists() {
        return Ok(sessions);
    }

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let filename = entry.file_name();
        let filename_str = filename.to_string_lossy();

        if filename_str.starts_with("s_") && filename_str.ends_with(".sock") {
            let session_name = &filename_str[2..filename_str.len() - 5];
            let sock_path = entry.path();
            let log_p = log_path(session_name);
            let pid_p = pid_path(session_name);

            let alive = is_socket_alive(&sock_path);
            let log_size = fs::metadata(&log_p).map(|m| m.len()).unwrap_or(0);
            let pid = fs::read_to_string(&pid_p)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok());

            sessions.push(SessionInfo {
                name: session_name.to_string(),
                socket_path: sock_path,
                log_path: log_p,
                is_alive: alive,
                log_size_bytes: log_size,
                pid,
            });
        }
    }

    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sessions)
}

/// Clean up dead session files (stale socket and pid files, keeping log).
#[allow(dead_code)]
pub fn cleanup_stale_session(name: &str) {
    let sock = socket_path(name);
    let pid = pid_path(name);

    if sock.exists() && !is_socket_alive(&sock) {
        let _ = fs::remove_file(&sock);
    }
    if pid.exists() {
        let _ = fs::remove_file(&pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_session_name() {
        assert!(sanitize_session_name("main").is_ok());
        assert!(sanitize_session_name("my-session_123").is_ok());
        assert!(sanitize_session_name("").is_err());
        assert!(sanitize_session_name("../evil").is_err());
        assert!(sanitize_session_name("session/name").is_err());
        assert!(sanitize_session_name("session name").is_err());
    }

    #[test]
    fn test_session_paths() {
        let name = "test_sess";
        let sock = socket_path(name);
        let log = log_path(name);
        let pid = pid_path(name);

        assert!(sock.to_string_lossy().ends_with("s_test_sess.sock"));
        assert!(log.to_string_lossy().ends_with("s_test_sess.log"));
        assert!(pid.to_string_lossy().ends_with("s_test_sess.pid"));
    }
}

