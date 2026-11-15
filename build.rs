//! Bundles the prelude: every file under `prelude/` goes into the binary (see src/prelude.rs).
//! The `.nu` files are loaded from the binary as they are; the other files (the runners of `py`
//! and `js`) are written out, to a folder named after a hash of them, so that changed runners
//! get a folder of their own.

use std::{
    fs,
    path::{Path, PathBuf},
};

fn files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir).expect("prelude/ is readable").filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            files(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("prelude");
    println!("cargo::rerun-if-changed={}", root.display());
    let mut found = Vec::new();
    files(&root, &mut found);

    // FNV-1a over the runners' names and contents: a name for this version of them.
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut modules = String::new();
    let mut runners = String::new();
    for path in &found {
        println!("cargo::rerun-if-changed={}", path.display());
        let relative = path.strip_prefix(&root).unwrap().to_string_lossy().into_owned();
        let entry = format!("    ({relative:?}, include_bytes!({:?})),\n", path.display().to_string());
        if relative.ends_with(".nu") {
            modules.push_str(&entry);
        } else {
            for byte in relative.bytes().chain(fs::read(path).unwrap()) {
                hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
            }
            runners.push_str(&entry);
        }
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("prelude_files.rs");
    fs::write(
        out,
        format!(
            "pub const MODULES: &[(&str, &[u8])] = &[\n{modules}];\n\
             pub const RUNNERS: &[(&str, &[u8])] = &[\n{runners}];\n\
             pub const RUNNERS_HASH: &str = \"{hash:016x}\";\n"
        ),
    )
    .unwrap();
}
