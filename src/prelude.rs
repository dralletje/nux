//! The prelude (nux's commands written in Nushell, and the runners of `py` and `js`), bundled
//! in the binary by build.rs.
//!
//! The `.nu` files are loaded from the binary, the way Nushell loads its standard library:
//! each is a virtual file under `nux/` (`nux/prelude.nu`, `nux/git/mod.nu`), so a call can
//! also `use nux/<module>` itself. Nothing of them is written to disk.
//!
//! Python and Node read their runners from files, so those are written out on first use, to a
//! folder named after their hash: a new nux with other runners gets a new folder, and nothing is
//! written again while they stay the same. Every call checks that each runner is still there,
//! with its size, and writes again what is not. The prelude finds the folder as the constant
//! `RUNNERS` of the virtual module `nux/runners.nu`.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use nu_protocol::{
    VirtualPathId,
    engine::{EngineState, StateWorkingSet, VirtualPath},
};

use crate::{paths::nux_dir, rewrite::nu_raw_string};

include!(concat!(env!("OUT_DIR"), "/prelude_files.rs"));

/// Make the prelude's modules available and give the code that loads the prelude.
///
/// With NUX_PRELUDE_DIR set (to work on the prelude itself without building nux), the modules
/// and the runners are read from that folder instead, under the same names.
pub fn install(engine: &mut EngineState) -> Result<String, String> {
    let from_disk = std::env::var_os("NUX_PRELUDE_DIR").filter(|d| !d.is_empty()).map(PathBuf::from);
    let (runners, mut modules) = match &from_disk {
        Some(dir) => {
            let mut found = Vec::new();
            read_modules(dir, dir, &mut found).map_err(|e| format!("Could not read the prelude in {}: {e}", dir.display()))?;
            (dir.clone(), found)
        }
        None => (runners_dir()?, MODULES.iter().map(|(name, contents)| (name.to_string(), contents.to_vec())).collect()),
    };
    let runners_module = format!("export const RUNNERS = {}\n", nu_raw_string(&runners.to_string_lossy()));
    modules.push(("runners.nu".into(), runners_module.into_bytes()));
    let modules: Vec<(&str, &[u8])> = modules.iter().map(|(name, contents)| (name.as_str(), contents.as_slice())).collect();

    let mut working_set = StateWorkingSet::new(engine);
    let _ = add_virtual_dir(&mut working_set, "nux", "", &modules);
    let delta = working_set.render();
    engine.merge_delta(delta).map_err(|e| format!("Could not load the prelude: {e}"))?;
    Ok("use nux/prelude *".to_string())
}

/// The `.nu` files under `dir`, by their path relative to `root`.
fn read_modules(root: &Path, dir: &Path, found: &mut Vec<(String, Vec<u8>)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            read_modules(root, &path, found)?;
        } else if path.extension().is_some_and(|e| e == "nu") {
            let relative = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            found.push((relative, std::fs::read(&path)?));
        }
    }
    Ok(())
}

/// The virtual folder `name` with the modules under `relative` (a folder in `prelude/`, "" for
/// the top), its subfolders first: a folder lists the ids of what is in it.
fn add_virtual_dir(working_set: &mut StateWorkingSet, name: &str, relative: &str, modules: &[(&str, &[u8])]) -> VirtualPathId {
    let mut entries = Vec::new();
    let mut subfolders = BTreeSet::new();
    for (path, contents) in modules {
        let rest = if relative.is_empty() {
            *path
        } else {
            match path.strip_prefix(relative).and_then(|p| p.strip_prefix('/')) {
                Some(rest) => rest,
                None => continue,
            }
        };
        match rest.split_once('/') {
            Some((folder, _)) => {
                subfolders.insert(folder);
            }
            None if rest == "mod.nu" => {
                let file_name = format!("{name}/mod.nu");
                let file_id = working_set.add_file(&file_name, contents);
                entries.push(working_set.add_virtual_path(file_name, VirtualPath::File(file_id)));
            }
            None => {
                // `git.nu` becomes the folder `nux/git` with a mod.nu: `use nux/git`, like `std/assert`.
                let module = format!("{name}/{}", rest.strip_suffix(".nu").unwrap_or(rest));
                let file_name = format!("{module}/mod.nu");
                let file_id = working_set.add_file(&file_name, contents);
                let file = working_set.add_virtual_path(file_name, VirtualPath::File(file_id));
                entries.push(working_set.add_virtual_path(module, VirtualPath::Dir(vec![file])));
            }
        }
    }
    for folder in subfolders {
        let inner = if relative.is_empty() { folder.to_string() } else { format!("{relative}/{folder}") };
        entries.push(add_virtual_dir(working_set, &format!("{name}/{folder}"), &inner, modules));
    }
    working_set.add_virtual_path(name.to_string(), VirtualPath::Dir(entries))
}

/// The folder with the runners, written out when it is not there yet.
fn runners_dir() -> Result<PathBuf, String> {
    let dir = nux_dir().join("runners").join(RUNNERS_HASH);
    if dir.exists() {
        repair(&dir)?;
        return Ok(dir);
    }
    // Written next to it and renamed into place: a call running at the same time sees all
    // of it or none of it.
    let temporary = nux_dir().join("runners").join(format!("{RUNNERS_HASH}.{}", std::process::id()));
    let write = || -> std::io::Result<()> {
        for (name, contents) in RUNNERS {
            let path = temporary.join(name);
            std::fs::create_dir_all(path.parent().unwrap())?;
            std::fs::write(path, contents)?;
        }
        Ok(())
    };
    write().map_err(|e| format!("Could not write the runners of py and js to {}: {e}", temporary.display()))?;
    if std::fs::rename(&temporary, &dir).is_err() {
        // Another call was first; its copy is the same.
        let _ = std::fs::remove_dir_all(&temporary);
    }
    repair(&dir)?;
    Ok(dir)
}

/// Write again every runner that is missing or has another size than the bundled one (the
/// temp directory gets cleaned up, partly or all at once). Each through a file of its own
/// renamed into place, so a call running at the same time never reads half a file. Size, not
/// contents: comparing contents would read every runner on every call.
fn repair(dir: &Path) -> Result<(), String> {
    for (name, contents) in RUNNERS {
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
