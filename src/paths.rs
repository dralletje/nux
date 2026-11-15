//! Where things are.

use std::path::PathBuf;

/// Large results and other files a call leaves for the next one. A fixed place (not the
/// caller's temp directory) so that every call of the same user finds them.
pub fn nux_dir() -> PathBuf {
    match std::env::var_os("NUX_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(format!("/tmp/nux-{}", unsafe { libc::getuid() })),
    }
}

/// The usage log: every call, one file per UTC day.
pub fn usage_log_dir() -> PathBuf {
    match std::env::var_os("NUX_LOG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home().join(".local/state/nux/log"),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}
