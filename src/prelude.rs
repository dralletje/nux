//! The prelude (nux's commands written in Nushell, and the runners of `py` and `js`), bundled
//! in the binary by build.rs. Nushell and Python read files, so on first use the prelude is
//! written out to a folder named after its hash: a new nux with another prelude gets a new
//! folder, and nothing is written again while it stays the same. Every call checks that each
//! file is still there, with its size, and writes again what is not.

use std::path::{Path, PathBuf};

use crate::paths::nux_dir;

include!(concat!(env!("OUT_DIR"), "/prelude_files.rs"));

/// The folder with the prelude: NUX_PRELUDE_DIR (to work on the prelude itself without
/// building nux), else the bundled one, written out when it is not there yet.
pub fn dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("NUX_PRELUDE_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let dir = nux_dir().join("prelude").join(HASH);
    if dir.exists() {
        repair(&dir)?;
        return Ok(dir);
    }
    // Written next to it and renamed into place: a call running at the same time sees all
    // of it or none of it.
    let temporary = nux_dir().join("prelude").join(format!("{HASH}.{}", std::process::id()));
    let write = || -> std::io::Result<()> {
        for (name, contents) in FILES {
            let path = temporary.join(name);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, contents)?;
        }
        Ok(())
    };
    write().map_err(|e| format!("Could not write the prelude to {}: {e}", temporary.display()))?;
    if std::fs::rename(&temporary, &dir).is_err() {
        // Another call was first; its copy is the same.
        let _ = std::fs::remove_dir_all(&temporary);
    }
    repair(&dir)?;
    Ok(dir)
}

/// Write again every file that is missing or has another size than the bundled one (the
/// temp directory gets cleaned up, partly or all at once). Each through a file of its own
/// renamed into place, so a call running at the same time never reads half a file. Size, not
/// contents: comparing contents would read the whole prelude on every call.
fn repair(dir: &Path) -> Result<(), String> {
    for (name, contents) in FILES {
        let path = dir.join(name);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() == contents.len() as u64) {
            continue;
        }
        let temporary = path.with_file_name(format!(".{}.{}", path.file_name().unwrap().to_string_lossy(), std::process::id()));
        std::fs::create_dir_all(path.parent().unwrap())
            .and_then(|_| std::fs::write(&temporary, contents))
            .and_then(|_| std::fs::rename(&temporary, &path))
            .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    }
    Ok(())
}
