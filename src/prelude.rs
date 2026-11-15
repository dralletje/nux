//! The prelude (nux's commands written in Nushell, and the runners of `py` and `js`), bundled
//! in the binary by build.rs.
//!
//! The `.nu` files are loaded from the binary, the way Nushell loads its standard library:
//! each is a virtual file under `nux/` (`nux/prelude.nu`, `nux/browser/mod.nu`), so a call can
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

use crate::{
    paths::nux_dir,
    rewrite::{CHECK_EXIT, nu_raw_string},
};

include!(concat!(env!("OUT_DIR"), "/prelude_files.rs"));

/// The prelude, ready to load: `source` loads it, `later` are its modules that it leaves out.
pub struct Prelude {
    pub source: String,
    pub later: Vec<Later>,
}

/// A module of the prelude that is not loaded yet. In its place is a stub: a command `stub`
/// that takes any arguments. Code that calls the stub needs the module, which `load` loads
/// (src/engine.rs). The stubs also keep the names known, for "Did you mean `wait-for`?".
/// They are in an overlay of their own (`stubs_source`), so that they can be put out of the
/// way all at once before modules are loaded: a module's examples are parsed with it, and
/// would find its stub (`browser do { ... }`: a stub takes no block). `hide` does not reach
/// into the modules parsed after it; `overlay hide` does.
#[derive(Clone)]
pub struct Later {
    pub stub: String,
    pub load: String,
}

/// Make the prelude's modules available and give the code that loads the prelude.
///
/// With `lazy`, the modules of prelude.nu are left out and stubs take their place (see
/// `Later`): parsing all of them takes longer than the rest of starting nux. Without, all of
/// them are loaded (`nux help`, `nux find`). `nux/output` is always loaded: nux adds its
/// `check-exit` to code itself (src/rewrite.rs).
///
/// With NUX_PRELUDE_DIR set (to work on the prelude itself without building nux), the modules
/// and the runners are read from that folder instead, under the same names.
pub fn install(engine: &mut EngineState, lazy: bool) -> Result<Prelude, String> {
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
    let mut later = Vec::new();
    if lazy {
        let module_text = |name: &str| {
            let file = format!("{name}.nu");
            let folder = format!("{name}/mod.nu");
            modules
                .iter()
                .find(|(path, _)| *path == file || *path == folder)
                .map(|(_, contents)| String::from_utf8_lossy(contents).into_owned())
        };
        let split = module_text("prelude").map(|prelude| with_stubs(&prelude, module_text));
        if let Some((trimmed, left_out)) = split
            && let Some((_, contents)) = modules.iter_mut().find(|(path, _)| path == "prelude.nu")
        {
            *contents = trimmed.into_bytes();
            later = left_out;
        }
    }
    let modules: Vec<(&str, &[u8])> = modules.iter().map(|(name, contents)| (name.as_str(), contents.as_slice())).collect();

    let mut working_set = StateWorkingSet::new(engine);
    let _ = add_virtual_dir(&mut working_set, "nux", "", &modules);
    let delta = working_set.render();
    engine.merge_delta(delta).map_err(|e| format!("Could not load the prelude: {e}"))?;
    Ok(Prelude { source: "use nux/prelude *".to_string(), later })
}

/// `prelude` with stubs in place of its `export use nux/<module>` lines, and what each stub
/// stands for. `module_text` gives a module's source by its name (`git`, `browser`).
fn with_stubs(prelude: &str, module_text: impl Fn(&str) -> Option<String>) -> (String, Vec<Later>) {
    let mut kept = String::new();
    let mut later = Vec::new();
    for line in prelude.lines() {
        let module = preloaded_module(line).and_then(|(module, star)| Some((module, star, module_text(module)?)));
        let Some((module, star, text)) = module else {
            kept.push_str(line);
            kept.push('\n');
            continue;
        };
        let name = module.rsplit('/').next().unwrap_or(module);
        let commands = exported_names(&text, name);
        if commands.iter().any(|c| c == CHECK_EXIT) {
            kept.push_str(line);
            kept.push('\n');
            continue;
        }
        // Without `*` its commands are `<module> ...`: one stub, under the module's name.
        let (stubs, load) =
            if star { (commands, format!("use nux/{module} *")) } else { (vec![name.to_string()], format!("use nux/{module}")) };
        for stub in stubs {
            later.push(Later { stub, load: load.clone() });
        }
    }
    (kept, later)
}

/// Code that puts stubs for `later` in place, as the overlay `overlay`.
pub fn stubs_source(later: &[Later], overlay: &str) -> String {
    let mut source = format!("module {overlay} {{\n");
    for Later { stub, .. } in later {
        source.push_str(&format!(
            "  export def --wrapped \"{stub}\" [...arguments] {{ error make {{msg: {}}} }}\n",
            nu_raw_string(&format!("nux did not load `{stub}` (a bug in nux, src/prelude.rs)")),
        ));
    }
    source.push_str(&format!("}}\noverlay use {overlay}\n"));
    source
}

/// Whether a command that code calls needs every command there is: it lists or looks up
/// commands, so the stubs would show instead of the real ones.
pub fn needs_everything(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    matches!(first, "help" | "scope" | "which" | "view")
}

/// `export use nux/git` gives ("git", false), `export use nux/servers *` ("servers", true).
fn preloaded_module(line: &str) -> Option<(&str, bool)> {
    let rest = line.trim().strip_prefix("export use nux/")?;
    match rest.split_whitespace().collect::<Vec<_>>().as_slice() {
        [module] => Some((module, false)),
        [module, "*"] => Some((module, true)),
        _ => None,
    }
}

/// The commands a module exports, by their name in it (`from tsc`; `main` is the module's
/// own name). Read from the text: enough for the prelude's own modules.
fn exported_names(text: &str, module: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim_start().strip_prefix("export ")?;
            let rest = rest.strip_prefix("def ").or_else(|| rest.strip_prefix("alias "))?;
            let rest = rest.trim_start();
            // Flags of def come before the name: export def --env name.
            let rest = rest.split_whitespace().skip_while(|w| w.starts_with("--")).collect::<Vec<_>>().join(" ");
            let name = match rest.chars().next()? {
                quote @ ('"' | '\'' | '`') => rest[1..].split(quote).next()?.to_string(),
                _ => rest.split(|c: char| c.is_whitespace() || c == '[' || c == '=').next()?.to_string(),
            };
            Some(if name == "main" { module.to_string() } else { name })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(name: &str) -> Option<String> {
        match name {
            "output" => Some("export def check-exit [] {}\nexport def show [] {}\n".into()),
            "servers" => Some("export def ports [] {}\nexport def --env \"wait-for\" [] {}\n".into()),
            "git" => Some("export def --wrapped main [...arguments] {}\nexport def status [] {}\n".into()),
            _ => None,
        }
    }

    #[test]
    fn modules_left_out_get_stubs() {
        let prelude = "use nux/runners RUNNERS\nexport use nux/output *\nexport use nux/git\nexport use nux/servers *\nexport def py [] {}\n";
        let (kept, later) = with_stubs(prelude, module);
        let stubs: Vec<_> = later.iter().map(|l| (l.stub.as_str(), l.load.as_str())).collect();
        assert_eq!(stubs, [("git", "use nux/git"), ("ports", "use nux/servers *"), ("wait-for", "use nux/servers *")]);
        assert_eq!(kept, "use nux/runners RUNNERS\nexport use nux/output *\nexport def py [] {}\n", "check-exit, which nux adds itself, stays");
        let source = stubs_source(&later, "nux-stubs");
        assert!(source.contains("  export def --wrapped \"wait-for\" [...arguments]"), "{source}");
        assert!(source.starts_with("module nux-stubs {\n") && source.ends_with("}\noverlay use nux-stubs\n"), "{source}");
    }
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
