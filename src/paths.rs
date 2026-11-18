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

/// nux's folder must be yours and nobody else's to write in: whoever can write in it can put
/// a prelude there that runs in your every call, and kept values (results, the variables of a
/// failed call) can hold secrets. So it is created owner-only (0700); a folder that is not
/// yours, or a link, is refused. A default folder (in /tmp) that others can read or write is
/// made owner-only; one you chose with NUX_DIR is only refused when others can write in it.
pub fn check_nux_dir() -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let dir = nux_dir();
    let chosen = std::env::var_os("NUX_DIR").is_some_and(|d| !d.is_empty());
    let _ = std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir);
    let meta = std::fs::symlink_metadata(&dir).map_err(|e| format!("nux cannot use its folder {}: {e}", dir.display()))?;
    let refuse = |why: String| format!("nux does not use {}: {why}. Remove it, or set NUX_DIR to a folder of your own.", dir.display());
    if !meta.file_type().is_dir() {
        return Err(refuse("it is not a folder (or it is a link)".into()));
    }
    let uid = unsafe { libc::getuid() };
    if meta.uid() != uid {
        return Err(refuse(format!("it belongs to another user (uid {})", meta.uid())));
    }
    let others_write = meta.mode() & 0o022 != 0;
    let others_read = meta.mode() & 0o055 != 0;
    if chosen && others_write {
        return Err(refuse("others can write in it (chmod go-w)".into()));
    }
    if !chosen && (others_write || others_read) {
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| refuse(format!("it is open to others and cannot be made owner-only ({e})")))?;
    }
    Ok(())
}

/// The user's own nux folder: `prelude.nu` (loaded in every call) and modules for
/// `use home/<name>`. `$XDG_CONFIG_HOME/nux`, else `~/.config/nux`; NUX_CONFIG_DIR instead.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("NUX_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    match std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir).join("nux"),
        None => home().join(".config/nux"),
    }
}

/// The project a folder is in: the nearest folder, from it upwards, with a `.git` (a folder in
/// a checkout, a file in a worktree or submodule).
pub fn project_root(from: &std::path::Path) -> Option<PathBuf> {
    from.ancestors().find(|dir| dir.join(".git").exists()).map(PathBuf::from)
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
