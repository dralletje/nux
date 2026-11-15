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

/// Make the prelude's modules available and give the code that loads the prelude.
///
/// With `code`, only the modules that code uses are loaded (see `needed`): parsing all of them
/// takes longer than the rest of starting nux. Without, all of them (`nux help`, `nux find`).
///
/// With NUX_PRELUDE_DIR set (to work on the prelude itself without building nux), the modules
/// and the runners are read from that folder instead, under the same names.
pub fn install(engine: &mut EngineState, code: Option<&str>) -> Result<String, String> {
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
    if let Some(code) = code {
        let module_text = |name: &str| {
            let file = format!("{name}.nu");
            let folder = format!("{name}/mod.nu");
            modules
                .iter()
                .find(|(path, _)| *path == file || *path == folder)
                .map(|(_, contents)| String::from_utf8_lossy(contents).into_owned())
        };
        let trimmed = module_text("prelude").map(|prelude| only_needed(&prelude, code, module_text));
        if let Some(trimmed) = trimmed
            && let Some((_, contents)) = modules.iter_mut().find(|(path, _)| path == "prelude.nu")
        {
            *contents = trimmed.into_bytes();
        }
    }
    let modules: Vec<(&str, &[u8])> = modules.iter().map(|(name, contents)| (name.as_str(), contents.as_slice())).collect();

    let mut working_set = StateWorkingSet::new(engine);
    let _ = add_virtual_dir(&mut working_set, "nux", "", &modules);
    let delta = working_set.render();
    engine.merge_delta(delta).map_err(|e| format!("Could not load the prelude: {e}"))?;
    Ok("use nux/prelude *".to_string())
}

/// `prelude` without the `export use nux/<module>` lines of the modules `code` does not need.
/// `module_text` gives a module's source by its name (`git`, `browser`).
fn only_needed(prelude: &str, code: &str, module_text: impl Fn(&str) -> Option<String>) -> String {
    let mut words = words(code);
    // Not in the code as given, but what nux adds after external commands (src/rewrite.rs).
    words.push(CHECK_EXIT.to_string());
    if needs_everything(&words) {
        return prelude.to_string();
    }
    prelude
        .lines()
        .filter(|line| {
            let Some((module, star)) = preloaded_module(line) else { return true };
            // Without `*` its commands are `<module> ...` (or `<module>`, its main).
            let name = module.rsplit('/').next().unwrap_or(module);
            if !star {
                return words.iter().any(|w| w == name);
            }
            match module_text(module) {
                Some(text) => exported_names(&text, name).iter().any(|command| mentions(&words, command)),
                None => true,
            }
        })
        .map(|line| format!("{line}\n"))
        .collect()
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

/// The words of code as Nushell names commands: letters, digits, `_` and `-`. `git hunks`
/// gives git, hunks; `^git` git; `--show` stays `--show`, a flag and not the command `show`.
fn words(code: &str) -> Vec<String> {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Code that needs every module: it loads a file nux does not see (`use`, `source`; not
/// `use nux/...` or `use std...`), or asks which commands there are.
fn needs_everything(words: &[String]) -> bool {
    words.iter().enumerate().any(|(i, word)| match word.as_str() {
        "use" => !matches!(words.get(i + 1).map(String::as_str), Some("nux" | "std" | "std-rfc")),
        "source" | "source-env" | "overlay" | "help" | "scope" | "which" | "view" => true,
        _ => false,
    })
}

/// The commands a module exports, by their name in it (`from tsc`; `main` is the module's
/// own name).
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

/// Whether the words of a command's name come one after the other in `words`.
fn mentions(words: &[String], command: &str) -> bool {
    let name: Vec<&str> = command.split_whitespace().collect();
    !name.is_empty() && words.windows(name.len()).any(|window| window.iter().zip(&name).all(|(a, b)| a == b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRELUDE: &str = "use nux/runners RUNNERS\nexport use nux/git\nexport use nux/servers *\nexport use nux/testing *\nexport def py [] {}\n";

    #[test]
    fn what_nux_adds_itself_is_always_loaded() {
        let prelude = "export use nux/output *\nexport use nux/git\n";
        let output = |name: &str| (name == "output").then(|| "export def check-exit [] {}\nexport def show [] {}\n".to_string());
        assert_eq!(only_needed(prelude, "ls", output), "export use nux/output *\n");
    }

    fn module(name: &str) -> Option<String> {
        match name {
            "servers" => Some("export def ports [] {}\nexport def --env \"wait-for\" [] {}\n".into()),
            "testing" => Some("export def \"from tap\" []: any -> table {}\n".into()),
            _ => None,
        }
    }

    fn kept(code: &str) -> Vec<String> {
        only_needed(PRELUDE, code, module).lines().filter_map(|l| l.strip_prefix("export use nux/")).map(str::to_string).collect()
    }

    #[test]
    fn only_the_modules_the_code_names_are_loaded() {
        assert_eq!(kept("ls | where size > 1mb"), Vec::<String>::new());
        assert_eq!(kept("git status | get path"), ["git"]);
        assert_eq!(kept("^git commit -m x"), ["git"]);
        assert_eq!(kept("let f = {|| wait-for 8080 }\ndo $f"), ["servers *"]);
        assert_eq!(kept("^node --test | complete | from tap"), ["testing *"]);
        assert_eq!(kept("open x.json | from json"), Vec::<String>::new(), "`from` alone is not `from tap`");
        assert_eq!(kept("ls | sort-by --reverse size"), Vec::<String>::new());
    }

    #[test]
    fn code_that_loads_files_or_lists_commands_gets_everything() {
        assert_eq!(kept("use .nu/d1.nu *\nd1 'select 1'"), ["git", "servers *", "testing *"]);
        assert_eq!(kept("scope commands | length"), ["git", "servers *", "testing *"]);
        assert_eq!(kept("use nux/cookie-jar\ncookie-jar new"), Vec::<String>::new());
        assert_eq!(kept("use std/assert"), Vec::<String>::new());
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
