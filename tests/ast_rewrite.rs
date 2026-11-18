//! `ast-find --rewrite | ast-apply`: the cases of tests/ast-rewrite (see its README), each
//! run through nux as an agent would. Needs `ast-grep`; skipped without.

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn has_ast_grep() -> bool {
    if Command::new("ast-grep").arg("--version").output().is_err() {
        eprintln!("skipped: no ast-grep");
        return false;
    }
    true
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nux-ast-rewrite-{}", std::process::id())).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// nux in `dir`, with values passed as environment variables (no quoting to get wrong).
fn nux(dir: &Path, code: &str, env: &[(&str, &str)]) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nux"))
        .env("NUX_LOG_DIR", dir.join("log"))
        .env("NUX_DIR", dir.join("nux"))
        .env("NUX_CONFIG_DIR", dir.join("config"))
        .envs(env.iter().copied())
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(code.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

const RUN_CASE: &str = "\
let rows = (ast-find --lang $env.CASE_LANG $env.CASE_PATTERN $env.CASE_FILE --rewrite $env.CASE_REWRITE)
$rows | ast-apply | ignore
{file: (open --raw $env.CASE_FILE), trailing: ($rows.0?.trailing?)} | to json --raw";

#[test]
fn every_case() {
    if !has_ast_grep() {
        return;
    }
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("ast-rewrite/cases.json")).expect("cases.json is JSON");
    assert!(cases.len() >= 30);
    let dir = scratch("cases");
    let mut failed = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let field = |name: &str| case[name].as_str().unwrap_or_else(|| panic!("case {index}: no {name}")).to_string();
        let file = dir.join(format!("case{index}.{}", field("ext")));
        std::fs::write(&file, field("source")).unwrap();
        let (status, stdout, stderr) = nux(
            &dir,
            RUN_CASE,
            &[
                ("CASE_LANG", &field("lang")),
                ("CASE_PATTERN", &field("pattern")),
                ("CASE_REWRITE", &field("rewrite")),
                ("CASE_FILE", file.to_str().unwrap()),
            ],
        );
        let name = field("name");
        if status != 0 {
            failed.push(format!("{name}: nux failed: {stdout}{stderr}"));
            continue;
        }
        let got: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        if got["file"] != case["expected"] {
            failed.push(format!("{name}: the file became {:?}, expected {:?}", got["file"], case["expected"]));
        }
        if got["trailing"] != case["trailing"] {
            failed.push(format!("{name}: trailing is {}, expected {}", got["trailing"], case["trailing"]));
        }
    }
    assert!(failed.is_empty(), "{} of {} cases failed:\n{}", failed.len(), cases.len(), failed.join("\n"));
}

/// The `;` is kept outside the match, so a replacement computed afterwards keeps it too.
#[test]
fn a_computed_replacement_keeps_the_trailing_part() {
    if !has_ast_grep() {
        return;
    }
    let dir = scratch("computed");
    std::fs::write(dir.join("t.ts"), "import { a } from \"./x\";\n").unwrap();
    let code = "ast-find --lang ts 'import { $$$A } from \"./x\"' t.ts --rewrite 'x'\n\
                | update replacement {|row| $\"import { ($row.vars.A | str join ', ') } from \\\"./computed\\\"\" }\n\
                | ast-apply | ignore\nopen --raw t.ts";
    let (status, stdout, stderr) = nux(&dir, code, &[]);
    assert_eq!(status, 0, "{stdout}{stderr}");
    assert_eq!(stdout, "import { a } from \"./computed\";\n");
}

/// Without --rewrite nothing changes: `text` is the whole match, and there is no `trailing`.
#[test]
fn a_search_without_rewrite_is_unchanged() {
    if !has_ast_grep() {
        return;
    }
    let dir = scratch("search");
    std::fs::write(dir.join("t.ts"), "import { a } from \"./x\";\n").unwrap();
    let (status, stdout, stderr) =
        nux(&dir, "ast-find --lang ts 'import { $$$A } from \"./x\"' t.ts | first | select text vars | to json --raw", &[]);
    assert_eq!(status, 0, "{stdout}{stderr}");
    assert_eq!(stdout, "{\"text\":\"import { a } from \\\"./x\\\";\",\"vars\":{\"A\":[\"a\"]}}\n");
    let (_, stdout, _) = nux(&dir, "ast-find --lang ts 'import { $$$A } from \"./x\"' t.ts | columns | to json --raw", &[]);
    assert!(!stdout.contains("trailing"), "{stdout}");
}

/// Anonymous holes get names for the search, which do not show in `vars`.
#[test]
fn anonymous_holes_stay_out_of_vars() {
    if !has_ast_grep() {
        return;
    }
    let dir = scratch("anonymous");
    std::fs::write(dir.join("t.ts"), "pick(a, b);\n").unwrap();
    let (status, stdout, stderr) =
        nux(&dir, "ast-find --lang ts 'pick($_, $B)' t.ts --rewrite 'x' | get 0.vars | to json --raw", &[]);
    assert_eq!(status, 0, "{stdout}{stderr}");
    assert_eq!(stdout, "{\"B\":\"b\"}\n");
}
