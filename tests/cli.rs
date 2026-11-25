//! nux as an agent uses it: code on stdin, the binary run as a process.

use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nux-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn nux(args: &[&str], code: &str) -> Run {
    nux_with(args, code, &[])
}

fn nux_with(args: &[&str], code: &str, env: &[(&str, &std::path::Path)]) -> Run {
    let dir = scratch();
    let mut child = Command::new(env!("CARGO_BIN_EXE_nux"))
        .args(args)
        .env("NUX_LOG_DIR", dir.join("log"))
        .env("NUX_DIR", dir.join("nux"))
        // Values as NUON: exact and typed, to compare. The readable form, nux's default, has
        // tests of its own (`values_are_printed_for_reading`), which set NUX_FORMAT back.
        .env("NUX_FORMAT", "nuon")
        // Not the user's own prelude and modules (~/.config/nux).
        .env("NUX_CONFIG_DIR", dir.join("config"))
        // Last, so that a test can override the ones above.
        .envs(env.iter().map(|(k, v)| (*k, *v)))
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(code.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn help_is_the_one_manual() {
    let help = nux(&["help"], "");
    assert_eq!(help.status, 0);
    for part in ["FROM BASH", "o+e>|", "-t application/json", "job recv"] {
        assert!(help.stdout.contains(part), "nux help lacks {part}");
    }
    assert!(!help.stdout.contains("nux guide"), "nux help still points to nux guide");
    // `nux guide` was folded into `nux help`.
    let guide = nux(&["guide"], "");
    assert_ne!(guide.status, 0);
    assert!(format!("{}{}", guide.stdout, guide.stderr).contains("Unknown command"));
}

/// Every session reads all of `nux help`: at most 12 KB, and still naming what agents use most
/// in the nux log, every module, and the user's own prelude (docs/compact-help.md).
#[test]
fn help_is_compact_and_keeps_what_agents_use() {
    let source = include_str!("../src/help.txt");
    assert!(source.len() <= 12_288, "src/help.txt is {} bytes", source.len());
    assert!(source.contains("{kept_dir}") && source.contains("{log_dir}"));
    for line in source.lines() {
        assert!(line.chars().count() <= 92, "too wide: {line}");
    }
    let help = nux(&["help"], "").stdout;
    for part in [
        "search", "--glob", "--context", "to grep", "complete", "^git", "glob ", "git log",
        "git status", "git diff", "git hunks", "git info", "git add", "git show-file", "kept",
        "edit-file", "with-env", "from tsc", "from json", "to md", "group-by", "from tap",
        "from cargo-test", "from bun-test", "ports", "wait-for url", "timeout", "| show",
        "check-exit", "try-nu", "nux help py", "nux help js", "cookie-jar", "capped",
        "nux/pnpm", "nux/npm", "into sqlite", "prelude.nu", "use home/", "use project/", "NUX_FORMAT",
        "-r '", "nux find", "nux-log",
    ] {
        assert!(help.contains(part), "nux help lacks {part}");
    }
}

/// What the compact help says matches what nux does, and the pitfalls that fail without an
/// error stay in it (the review of the compact help, 2026-11-24).
#[test]
fn help_says_what_nux_does() {
    let help = nux(&["help"], "").stdout;
    for part in [
        "inside a table, record or list", // top-level text is never quoted
        "list of texts",                  // [1 2 3] is NUON, not `3 items`
        "durations, sizes",
        "--stable 5sec",                  // --stable takes a duration
        "| complete } | from tap",
        "name, size, kept",               // the columns of `kept`
        "unless you pipe",                // externals get stdin when piped
        "\"hello $name\" stays as written",
        "detect columns",
        "column `trailing`",
        "--exclude",
        "glob '{src,test}/**/*.ts'", // braces need quotes, or it is a parse error
        "not by a relative path",
    ] {
        assert!(help.contains(part), "nux help lacks {part}");
    }
    // `git status --short` is a table in nux (prelude/git.nu), not an error.
    assert!(!help.contains("`git status --short`) is an error"));
}

/// `nux help capped` is what an agent reads: what it does and returns, not "this file".
#[test]
fn help_capped_says_what_it_returns() {
    let run = nux(&["help", "capped"], "");
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    assert!(!run.stdout.contains("this file"), "{}", run.stdout);
    for part in ["killed_by", "peak_memory", "capped --mem 1500MB --time 40sec pnpm exec tsc -b"] {
        assert!(run.stdout.contains(part), "nux help capped lacks {part}:\n{}", run.stdout);
    }
}

/// The nux-only hook's own tests (hooks/test_nux_only.py), so CI runs them too.
#[test]
fn nux_only_hook_lets_through_what_it_should() {
    let run = Command::new("python3").arg(concat!(env!("CARGO_MANIFEST_DIR"), "/hooks/test_nux_only.py")).output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
}

/// nux's default form, made for reading (src/readable.rs, "v0" in docs/output-format-eval.md).
#[test]
fn values_are_printed_for_reading() {
    let readable = std::path::Path::new("readable");
    let run = |code: &str| {
        let run = nux_with(&[], code, &[("NUX_FORMAT", readable)]);
        assert_eq!(run.status, 0, "{code}: {}{}", run.stdout, run.stderr);
        run.stdout
    };
    // A table: Markdown; a list of texts: `- item`, quoted only when it could be misread.
    assert_eq!(run("[[a b]; [1 x] [2 'y|z']]"), "| a | b |\n|---|---|\n| 1 | x |\n| 2 | \"y|z\" |\n");
    // A list of texts: how many, then one per line. ("two\nlines" has a newline; 'let s =
    // "a\nb";' a backslash and an n, as code does; spaces at the start stay as they are.)
    assert_eq!(
        run("[one \"two\\nlines\" '12' '' 'let s = \"a\\nb\";' '    indented' 'trailing ']"),
        "7 items\none\n\"two\\nlines\"\n\"12\"\n\"\"\nlet s = \"a\\nb\";\n    indented\n\"trailing \"\n"
    );
    assert_eq!(run("[only]"), "1 item\nonly\n");
    // A record: `key: value`; durations and sizes for reading; short nested values inline.
    assert_eq!(
        run("{took: 1500ms, size: 4512345b, at: 2026-11-17T14:02:11+00:00, files: [a b]}"),
        "took: 1sec 500ms\nsize: ~4.512MB\nat: 2026-11-17T14:02:11+00:00\nfiles: [a,b]\n"
    );
    // A text over several lines in a record: a fenced block under its key.
    assert_eq!(run("{message: \"first\\n\\nsecond\"}"), "message:\n```\nfirst\n\nsecond\n```\n");
    // A long nested value: indented under its key.
    let long = run("{nested: {items: (1..30 | each { $\"item($in)\" })}}");
    assert!(long.starts_with("nested:\n  items:\n    - item1\n    - item2\n"), "{long}");
    // A table that is not simple (a nested cell): a record per row, the number of rows above.
    assert_eq!(run("[[a b]; [1 [x]]]"), "1 row\n## 0\na: 1\nb: [x]\n");
    assert_eq!(run("[[a b]; [1 [x]] [2 [y]]]"), "2 rows\n## 0\na: 1\nb: [x]\n\n## 1\na: 2\nb: [y]\n");
    // The rest stays NUON.
    assert_eq!(run("[1 2 3]"), "[1,2,3]\n");
    assert_eq!(run("0..<10"), "0..<10\n");
    // `show` and the sample of a large result print the same form.
    assert_eq!(run("[[a b]; [1 x]] | show | ignore"), "| a | b |\n|---|---|\n| 1 | x |\n");
    let large = run("1..5000 | each {|i| {i: $i, s: 'some text'} }");
    assert!(large.contains("| i | s |\n|---|---|\n| 1 | some text |"), "{large}");
    // NUX_FORMAT=nuon: NUON for everything, as before.
    assert_eq!(nux_with(&[], "[[a b]; [1 x]]", &[("NUX_FORMAT", std::path::Path::new("nuon"))]).stdout, "[[a,b];[1,x]]\n");
}

#[test]
fn data_comes_back_as_nuon() {
    let run = nux(&[], "1 + 1");
    assert_eq!((run.status, run.stdout.as_str()), (0, "2\n"));
    let run = nux(&[], "[[a b]; [1 x] [2 y]]");
    assert_eq!(run.stdout, "[[a,b];[1,x],[2,y]]\n");
}

#[test]
fn lists_of_one_stay_lists_and_no_value_prints_nothing() {
    assert_eq!(nux(&[], "[1]").stdout, "[1]\n");
    assert_eq!(nux(&[], "let x = 1").stdout, "");
    assert_eq!(nux(&[], "[]").stdout, "[]\n");
}

#[test]
fn text_comes_back_as_it_is() {
    assert_eq!(nux(&[], "'two\nlines'").stdout, "two\nlines\n");
    assert_eq!(nux(&[], "{a: 1} | to json --raw").stdout, "{\"a\":1}\n");
}

#[test]
fn printed_text_comes_before_the_value() {
    assert_eq!(nux(&[], "print 'first'; 2").stdout, "first\n2\n");
}

#[test]
fn the_last_external_command_shows_its_output_and_fails_the_call() {
    let run = nux(&[], "^sh -c 'echo out; echo warn >&2; exit 3'");
    assert_eq!(run.status, 1);
    assert_eq!(
        run.stdout,
        "out\nstderr (^sh -c 'echo out; echo warn >&2; exit 3'): warn\nError: `^sh -c 'echo out; echo warn >&2; exit 3'` exited with code 3\n"
    );
    let run = nux(&[], "^false");
    assert_eq!(run.status, 1);
}

#[test]
fn an_earlier_failing_command_shows_its_output_and_ends_the_call() {
    let run = nux(&[], "^sh -c 'echo broken; exit 2'\n'not reached'");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("broken"), "{}", run.stdout);
    assert!(run.stdout.contains("exited with code 2"), "{}", run.stdout);
    assert!(!run.stdout.contains("not reached"), "{}", run.stdout);
}

#[test]
fn an_earlier_command_that_succeeds_shows_its_output() {
    let run = nux(&[], "^sh -c 'echo out; echo seen >&2'\n42");
    assert_eq!(
        (run.status, run.stdout.as_str()),
        (
            0,
            "stderr (^sh -c 'echo out; echo seen >&2'): seen\n── line 1: ^sh -c 'echo out; echo seen >&2'\nout\n── line 2: 42\n42\n"
        )
    );
    // Muted: no output, so no lines naming statements either.
    let run = nux(&[], "^sh -c 'echo out' o> /dev/null\n42");
    assert_eq!(run.stdout, "42\n");
}

/// Every statement's value is shown, each under a line naming it; one value alone as before.
#[test]
fn every_value_is_shown() {
    let run = nux(&[], "1 + 1\nlet x = 5\n[a b]\n$x");
    assert_eq!(run.stdout, "── line 1: 1 + 1\n2\n── line 3: [a b]\n[a,b]\n── line 4: $x\n5\n");
    // Declarations, assignments, loops, print, cd and ignore give nothing to show.
    let run = nux(
        &[],
        "def f [] { 1 }\nmut y = 1\n$y = 2\nfor i in [1] { $i }\nprint 'p'\ncd /\nls | ignore\nuse std/assert\n$y",
    );
    assert_eq!(run.stdout, "p\n2\n");
    // if, match and try give values.
    let run = nux(&[], "if true { 'yes' }\ntry { 'tried' }\n3");
    assert_eq!(run.stdout, "── line 1: if true { 'yes' }\nyes\n── line 2: try { 'tried' }\ntried\n── line 3: 3\n3\n");
    // A def --env still changes the environment, and `let` still gets its own value.
    let run = nux(&[], "def --env go [] { $env.Z = 'z' }\ngo\nlet v = 7\n$\"($env.Z)($v)\"");
    assert_eq!(run.stdout, "z7\n");
    // Several statements on one line.
    let run = nux(&[], "1; 2");
    assert_eq!(run.stdout, "── line 1: 1\n1\n── line 1: 2\n2\n");
}

/// An external that fails inside a def, its output going on into the pipeline: the call fails
/// (it used to succeed), with the value the pipeline still gave. Into a value (`let x = (b)`)
/// Nushell drops the output: a hint says how to keep it.
#[test]
fn a_failing_external_inside_a_def_fails_the_call() {
    let def = "def b [] { ^sh -c 'echo out-text; exit 3' }\n";
    let run = nux(&[], &format!("{def}b | lines"));
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains("[out-text]") && run.stdout.contains("exited with code 3"), "{}", run.stdout);
    let run = nux(&[], &format!("{def}let x = (b)\n$x"));
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains("| complete"), "{}", run.stdout);
    let run = nux(&[], "def b [] { ^sh -c 'echo fine' }\nb | lines");
    assert_eq!((run.status, run.stdout.as_str()), (0, "[fine]\n"), "{}", run.stderr);
}

#[test]
fn errors_point_at_the_code() {
    let run = nux(&[], "let a = 1\nopen /nope.json");
    assert_eq!(run.status, 1);
    assert!(run.stdout.starts_with("Error: "), "{}", run.stdout);
    assert!(run.stdout.contains("line 2, column 6"), "{}", run.stdout);
}

#[test]
fn parse_errors_point_at_the_code() {
    let run = nux(&[], "let x = (1 +");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("nu::parser::"), "{}", run.stdout);
    assert!(run.stdout.contains("line 1"), "{}", run.stdout);
}

#[test]
fn the_prelude_is_there() {
    let run = nux(&[], "'a,b' | split row ',' | show { length } | last");
    assert_eq!((run.status, run.stdout.as_str()), (0, "2\nb\n"), "{}", run.stderr);
}

#[test]
fn the_prelude_comes_from_the_binary() {
    // A NUX_DIR of its own: this test damages the prelude on purpose, which must not reach
    // the other tests running at the same time.
    let own = scratch().join(format!("prelude-test-{}", std::process::id()));
    let env = [("NUX_DIR", own.as_path())];
    let nux = |code: &str| nux_with(&[], code, &env);
    // `py` needs its runner as a file: written out from the binary into NUX_DIR. The .nu
    // files are loaded from the binary and not written.
    let run = nux("[1 2 3] | py 'sum(data)'");
    assert_eq!((run.status, run.stdout.as_str()), (0, "6\n"), "{}", run.stderr);
    let folders: Vec<_> = std::fs::read_dir(own.join("runners")).unwrap().filter_map(Result::ok).collect();
    assert_eq!(folders.len(), 1, "one folder, named after the runners' hash");
    let dir = folders[0].path();
    assert!(dir.join("py_runner.py").exists());
    assert!(dir.join("js_runner.mjs").exists());
    let written: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|e| e.file_name()).collect();
    assert!(!written.iter().any(|name| name.to_string_lossy().ends_with(".nu")), "{written:?}");
    assert!(!own.join("prelude").exists());

    // Part of it removed (a temp cleanup), or cut short: written again on the next call.
    std::fs::remove_file(dir.join("py_runner.py")).unwrap();
    std::fs::write(dir.join("nuon.py"), "").unwrap();
    let run = nux("[1 2 3] | py 'sum(data)'");
    assert_eq!((run.status, run.stdout.as_str()), (0, "6\n"), "{}{}", run.stdout, run.stderr);

    // The prelude's modules can be loaded by name.
    let run = nux("use nux/cookie-jar; scope commands | where name == 'cookie-jar new' | length");
    assert_eq!((run.status, run.stdout.as_str()), (0, "1\n"), "{}{}", run.stdout, run.stderr);
    let run = nux("try-nu 'use nux/typescript;\"a.ts(1,2): error TS1: x\" | typescript from tsc | get code.0' | get value");
    assert_eq!((run.status, run.stdout.as_str()), (0, "TS1\n"), "{}{}", run.stdout, run.stderr);
}

/// `http` against a server of the test's own (no network needed): JSON comes back parsed.
#[test]
fn http_works() {
    use std::io::{Read, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(1) {
            let mut stream = stream.unwrap();
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let body = r#"{"name":"nux","n":2}"#;
            let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    let run = nux(&[], &format!("http get http://127.0.0.1:{port}/ | get n"));
    assert_eq!((run.status, run.stdout.as_str()), (0, "2\n"), "{}", run.stdout);
}

/// The modules that are not preloaded load with `use nux/<name>` and give their commands
/// under that name, without `*`; `edit-file` is preloaded. cookie-jar against a server of the
/// test's own: a login that sets a cookie and redirects, then a page that shows the cookie.
#[test]
fn modules_to_use_and_edit_file() {
    use std::io::{Read, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            let mut stream = stream.unwrap();
            let mut request = [0u8; 4096];
            let read = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..read]).to_string();
            if request.starts_with("POST /login") {
                let _ = write!(stream, "HTTP/1.1 303 See Other\r\nLocation: /me\r\nSet-Cookie: session=abc; Path=/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            } else {
                let cookie = request.lines().find_map(|l| l.strip_prefix("cookie: ").or(l.strip_prefix("Cookie: "))).unwrap_or("none").to_string();
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cookie}", cookie.len());
            }
        }
    });
    let run = nux(
        &[],
        &format!(
            "use nux/cookie-jar\nlet jar = (cookie-jar new)\ncookie-jar http $jar post http://127.0.0.1:{port}/login {{a: 1}} -t application/json | get body\n"
        ),
    );
    assert_eq!((run.status, run.stdout.as_str()), (0, "session=abc\n"), "{}{}", run.stdout, run.stderr);

    // Nushell's own `update` and `http` are untouched by `use nux/cookie-jar`.
    let run = nux(&[], "use nux/cookie-jar\nuse nux/capped\n[{a: 1}] | update a 2 | get a.0");
    assert_eq!((run.status, run.stdout.as_str()), (0, "2\n"), "{}{}", run.stdout, run.stderr);
    let run = nux(&[], "use nux/capped\ncapped");
    assert!(run.stdout.contains("capped: no command given"), "{}{}", run.stdout, run.stderr);

    let file = scratch().join(format!("edit-file-{}.txt", std::process::id()));
    std::fs::write(&file, "let a = 1\nlet b = 1\n").unwrap();
    let run = nux(&[], &format!("edit-file '{}' 'let a = 1' 'let a = 2' | get replacements", file.display()));
    assert_eq!((run.status, run.stdout.as_str()), (0, "1\n"), "{}{}", run.stdout, run.stderr);
    let run = nux(&[], &format!("edit-file '{}' ' = ' ' := '", file.display()));
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains("matches 2 times"), "{}", run.stdout);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "let a = 2\nlet b = 1\n");
}

/// https needs a TLS crypto provider set at startup. Without one the request fails before it
/// connects; with one it fails on the connection (nothing listens on port 1).
#[test]
fn https_has_a_crypto_provider() {
    let run = nux(&[], "http get https://127.0.0.1:1/");
    assert_eq!(run.status, 1);
    assert!(!run.stdout.contains("crypto provider"), "{}", run.stdout);
}

/// A git repository of the test's own: two commits (README.md, sub/a.txt, `a..b.txt` and
/// old.txt in the first; README.md changed in the second), then, not committed: README.md
/// and `a..b.txt` changed, old.txt deleted.
fn git_repo(name: &str) -> PathBuf {
    let dir = scratch().join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(&dir)
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    for (file, text) in [("README.md", "one\n"), ("sub/a.txt", "a\n"), ("a..b.txt", "dots\n"), ("old.txt", "old\n")] {
        std::fs::write(dir.join(file), text).unwrap();
    }
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);
    std::fs::write(dir.join("README.md"), "one\ntwo\n").unwrap();
    git(&["commit", "-q", "-am", "second"]);
    std::fs::write(dir.join("README.md"), "one\ntwo\nthree\n").unwrap();
    std::fs::write(dir.join("a..b.txt"), "dots\nmore\n").unwrap();
    std::fs::remove_file(dir.join("old.txt")).unwrap();
    dir
}

fn nux_in(dir: &std::path::Path, code: &str) -> Run {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nux"))
        .env("NUX_LOG_DIR", scratch().join("log"))
        .env("NUX_DIR", scratch().join("nux"))
        // Exact values to compare, without the user's own prelude (see nux_with).
        .env("NUX_FORMAT", "nuon")
        .env("NUX_CONFIG_DIR", scratch().join("config"))
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(code.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `search` takes paths as paths: `~` is the home directory.
#[test]
fn search_expands_the_home_directory() {
    let home = scratch().join(format!("home-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("note.txt"), "needle\n").unwrap();
    let run = nux_with(&[], "search needle ~/note.txt | length", &[("HOME", home.as_path())]);
    assert_eq!((run.status, run.stdout.as_str()), (0, "1\n"), "{}{}", run.stdout, run.stderr);
}

/// nux's table commands give every column in every row (null where there is nothing), so that
/// `where column != null` works without `column?` (AGENTS.md). On a repository with a staged,
/// an unstaged, a deleted and an untracked file, and on tsc and TAP output with different kinds
/// of rows.
#[test]
fn table_commands_give_every_column_in_every_row() {
    let repo = git_repo("columns");
    std::fs::write(repo.join("new.txt"), "new\n").unwrap();
    let code = r##"^git add README.md
let tsc = "a.ts(1,7): error TS2322: Type 'string' is not assignable to type 'number'.\nerror TS5083: Cannot read file 'tsconfig.json'.\n"
let tap = "TAP version 13\nok 1 - first\nnot ok 2 - second\n  ---\n  error: boom\n  ...\n1..2\n"
let tables = {
  "git status": (git status)
  "git diff": (git diff)
  "git diff --lines": (git diff --lines)
  "git log --files --body": (git log --files --body)
  "search": (search 'one|old|dots' .)
  "search -C 1": (search 'two' . -C 1)
  "search --files": (search 'o' . --files)
  "ports": (try { ports } catch { [] })
  "from tsc": ($tsc | from tsc)
  "from tap": ($tap | from tap)
}
$tables
| transpose command rows
| where {|t| ($t.rows | each { columns | str join ',' } | uniq | length) > 1 }
| get command
| to nuon"##;
    let run = nux_in(&repo, code);
    assert_eq!((run.status, run.stdout.as_str()), (0, "[]\n"), "commands with rows of different columns: {}{}", run.stdout, run.stderr);
}

/// `git status | git add` stages what is left, also when a deletion is already staged (git
/// does not find that file any more).
#[test]
fn git_add_takes_git_status_with_staged_deletions() {
    let repo = git_repo("git-add");
    let run = nux_in(&repo, "^git rm -q old.txt\ngit status | git add | where unstaged != null | length");
    assert_eq!((run.status, run.stdout.as_str()), (0, "0\n"), "{}{}", run.stdout, run.stderr);
    let run = nux_in(&repo, "git status | select path staged | to nuon");
    assert_eq!(
        run.stdout,
        "[[path, staged]; [\"README.md\", modified], [\"a..b.txt\", modified], [\"old.txt\", deleted]]\n",
        "{}",
        run.stderr
    );

    // As arguments, a glob is for git to read, and `git log` takes paths too.
    let repo = git_repo("git-add-glob");
    let run = nux_in(&repo, "git add '*.md' | where staged != null | get path | to nuon");
    assert_eq!((run.status, run.stdout.as_str()), (0, "[\"README.md\"]\n"), "{}", run.stderr);
    let run = nux_in(&repo, "git log sub | length");
    assert_eq!((run.status, run.stdout.as_str()), (0, "1\n"), "{}", run.stderr);
}

/// `git diff` tells revisions from paths, also for paths with `..` in them.
#[test]
fn git_diff_tells_ranges_from_paths() {
    let repo = git_repo("git-diff");
    let sub = repo.join("sub");
    // A path from a subdirectory, as `git status` gives it there: a path, not a range.
    // (Paths joined with spaces: plain text, no NUON quoting to get right.)
    let paths = |dir: &std::path::Path, code: &str| {
        let run = nux_in(dir, &format!("{code} | get path | sort | str join ' '"));
        assert_eq!(run.status, 0, "{code}: {}{}", run.stdout, run.stderr);
        run.stdout.trim_end().to_string()
    };
    assert_eq!(paths(&sub, "git diff ../README.md"), "../README.md");
    // `..` alone: the parent directory, here the whole repository.
    assert_eq!(paths(&sub, "git diff .."), "../README.md ../a..b.txt ../old.txt");
    assert_eq!(paths(&sub, "git diff ...(git status | get path)"), "../README.md ../a..b.txt ../old.txt");
    // A deleted file (not on disk) and a file name with two dots in it.
    assert_eq!(paths(&sub, "git diff ../old.txt"), "../old.txt");
    assert_eq!(paths(&repo, "git diff a..b.txt"), "a..b.txt");
    // Ranges: both ends commits; an empty end is HEAD.
    for range in ["HEAD~1..HEAD", "HEAD~1..", "HEAD~1...HEAD", "main~1..main"] {
        assert_eq!(paths(&repo, &format!("git diff {range}")), "README.md", "{range}");
    }
    // A range and a path together.
    let run = nux_in(&repo, "git diff HEAD~1..HEAD README.md | select path added");
    assert_eq!(run.stdout, "[[path,added];[\"README.md\",1]]\n", "{}{}", run.stdout, run.stderr);
}

/// `git diff --lines` and `git hunks` read paths from the diff's headers; git's config must
/// not change those (`diff.noprefix` made `README.md` into `ADME.md`).
#[test]
fn git_diff_paths_do_not_depend_on_git_config() {
    let repo = git_repo("git-noprefix");
    let status = Command::new("git").args(["config", "diff.noprefix", "true"]).current_dir(&repo).status().unwrap();
    assert!(status.success());
    let run = nux_in(&repo, "git diff --lines README.md | get path | uniq | str join ' '");
    assert_eq!(run.stdout, "README.md\n", "{}{}", run.stdout, run.stderr);
    let run = nux_in(&repo, "git diff | where path == README.md | git hunks | get path | str join ' '");
    assert_eq!(run.stdout, "README.md\n", "{}{}", run.stdout, run.stderr);
}

/// `from tsc` on real tsc output (tests/fixtures/tsc5: TypeScript 5.9.3, tsc7: 7.0.2, the Go
/// rewrite): plain, as through `complete`; with --pretty (colors, code excerpts, related
/// locations); an error about no file; and build mode. Plain output is the same in both
/// versions; --pretty is not (where related locations go, the summary of a build), and both
/// must give the same rows.
#[test]
fn from_tsc_reads_what_tsc_prints() {
    for version in ["tsc5", "tsc7"] {
        from_tsc_reads(version);
    }
}

fn from_tsc_reads(version: &str) {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(version);
    let rows = |file: &str| {
        let run = nux(&[], &format!("open --raw '{}' | from tsc | to json --raw", fixtures.join(file).display()));
        assert_eq!(run.status, 0, "{version}/{file}: {}{}", run.stdout, run.stderr);
        serde_json::from_str::<serde_json::Value>(&run.stdout).unwrap()
    };
    let plain = rows("plain.txt");
    assert_eq!(plain.as_array().unwrap().len(), 3);
    assert_eq!(
        plain[0],
        serde_json::json!({"file": "a.ts", "line": 1, "column": 7, "category": "error", "code": "TS2322",
            "message": "Type 'string' is not assignable to type 'number'."})
    );
    // A message over three lines, kept whole.
    assert_eq!(
        plain[2]["message"],
        "Argument of type '{ a: number; }' is not assignable to parameter of type '{ a: string; }'.\n\
         Types of property 'a' are incompatible.\nType 'number' is not assignable to type 'string'."
    );
    assert_eq!(rows("pretty.txt"), plain, "{version}: --pretty gives the same rows");
    let global = rows("global.txt");
    assert_eq!(global[0]["file"], serde_json::Value::Null);
    assert_eq!(global[0]["code"], "TS5058");

    // Build mode (`tsc -b --force`) over two projects with references, files in
    // subdirectories. With --pretty, TypeScript 7 ends in an indented summary table
    // ("Errors  Files"), which is not part of any message.
    let build = rows("build-plain.txt");
    let places: Vec<String> =
        build.as_array().unwrap().iter().map(|row| format!("{}:{} {}", row["file"].as_str().unwrap(), row["line"], row["code"].as_str().unwrap())).collect();
    assert_eq!(places, ["lib/src/index.ts:1 TS2322", "app/src/main.ts:2 TS2345", "app/src/main.ts:3 TS2322", "app/src/other.ts:1 TS2322"]);
    assert_eq!(rows("build-pretty.txt"), build, "{version}: --pretty gives the same rows, without the summary");
}

/// `from cargo-test` reads what `cargo test` printed (fixtures: a crate with a passing, a
/// failing, a panicking and an ignored test, and a second test binary; and one that does not
/// compile, which must be an error, never an empty table).
#[test]
fn from_cargo_test_reads_what_cargo_test_prints() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cargo-test");
    let record = |name: &str| {
        format!(
            "{{stdout: (open --raw '{0}/{name}.stdout.txt'), stderr: (open --raw '{0}/{name}.stderr.txt'), exit_code: 101}}",
            fixtures.display()
        )
    };
    let run = nux(&[], &format!("{} | from cargo-test | to json --raw", record("failing")));
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    let rows: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    let summary: Vec<String> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| format!("{} {} {}:{}", r["name"].as_str().unwrap(), r["status"].as_str().unwrap(), r["file"], r["line"]))
        .collect();
    assert_eq!(
        summary,
        [
            "tests::slow skip null:null",
            "tests::adds pass null:null",
            "tests::adds_wrong fail \"src/lib.rs\":12",
            "tests::panics_with_lines fail \"src/lib.rs\":21",
            "from_outside pass null:null",
        ]
    );
    assert_eq!(rows[2]["error"], "assertion `left == right` failed: one and two\n  left: 3\n right: 4");
    assert_eq!(rows[3]["error"], "first line\nsecond line");
    let run = nux(&[], &format!("{} | from cargo-test", record("compile-error")));
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains("from cargo-test: this is not what cargo test prints"), "{}", run.stdout);
    assert!(run.stdout.contains("expected expression, found"), "{}", run.stdout);
}

/// `from bun-test` reads what `bun test` printed (fixtures from Bun 1.3: the full report;
/// the same run where Bun listed only failures, as it does with CLAUDECODE=1; and a run with
/// a syntax error, which must be an error, never an empty table).
#[test]
fn from_bun_test_reads_what_bun_test_prints() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bun-test");
    let record = |name: &str| {
        format!(
            "{{stdout: (open --raw '{0}/{name}.stdout.txt'), stderr: (open --raw '{0}/{name}.stderr.txt'), exit_code: 1}}",
            fixtures.display()
        )
    };
    let run = nux(&[], &format!("{} | from bun-test | each {{|r| $'($r.status) ($r.name) ($r.line)' }} | str join (char nl)", record("full")));
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        "pass strings \npass math > adds \nfail math > nested > multiplies wrong 9\nskip math > skipped one \ntodo math > not written yet \nfail throws 17\n"
    );
    let run = nux(&[], &format!("{} | from bun-test | where status == fail | get error | to json --raw", record("full")));
    assert_eq!(run.stdout, "[\"expect(received).toBe(expected)\\n\\nExpected: 7\\nReceived: 6\",\"first line\\nsecond line\"]\n");
    // Only failures listed: those rows, and a note that passing ones are missing.
    let run = nux(&[], &format!("{} | from bun-test | length", record("failing")));
    assert_eq!(run.stdout, "2\n");
    assert!(run.stderr.contains("2 of 2 passing tests are not in the table"), "{}", run.stderr);
    let run = nux(&[], &format!("{} | from bun-test", record("broken")));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("this is not a bun test report") && run.stdout.contains("Unexpected end of file"), "{}", run.stdout);
}

/// `from swift-test` reads what `swift test` printed, XCTest and swift-testing both (fixtures
/// from Swift 6.4 on Linux; a build that failed must be an error, never an empty table).
#[test]
fn from_swift_test_reads_what_swift_test_prints() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/swift-test");
    let record = |name: &str| {
        format!(
            "{{stdout: (open --raw '{0}/{name}.stdout.txt'), stderr: (open --raw '{0}/{name}.stderr.txt'), exit_code: 1}}",
            fixtures.display()
        )
    };
    let run = nux(&[], &format!("{} | from swift-test | each {{|r| $'($r.status) ($r.name) ($r.line)' }} | str join (char nl)", record("failing")));
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        "pass MathTests > testAdds \nfail MathTests > testAddsWrong 6\nskip MathTests > testSkipped \nskip later() \npass joins() \nfail topLevel() 10\nfail joinsWrong() 6\n"
    );
    let run = nux(&[], &format!("{} | from swift-test | where status == fail | get error | to json --raw", record("failing")));
    assert_eq!(
        run.stdout,
        "[\"XCTAssertEqual failed: (\\\"3\\\") is not equal to (\\\"4\\\") - one and two\",\"Caught error: first line\\nsecond line\",\"Expectation failed: \\\"a\\\" + \\\"b\\\" == \\\"ba\\\"\\n↳ order matters\\n↳ \\\"a\\\" + \\\"b\\\" == \\\"ba\\\" → false\\n↳   \\\"a\\\" + \\\"b\\\" → \\\"ab\\\"\"]\n"
    );
    let run = nux(&[], &format!("{} | from swift-test", record("compile-error")));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("this is not what swift test prints") && run.stdout.contains("Build failed"), "{}", run.stdout);
}

/// `from tsc` takes the record of `complete`, reads both streams, and fails when the command
/// failed without a diagnostic: `[]` must mean "no errors", never "tsc did not run". From
/// `pnpm -r`, the package folder before each line becomes part of the file.
#[test]
fn from_tsc_takes_complete_and_never_hides_a_failure() {
    let plain = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tsc5/plain.txt");
    let plain = format!("(open --raw '{}')", plain.display());
    let run = nux(&[], &format!("{{stdout: '', stderr: {plain}, exit_code: 2}} | from tsc | length"));
    assert_eq!(run.stdout, "3\n", "diagnostics on stderr: {}", run.stdout);
    let run = nux(&[], "{stdout: '', stderr: '', exit_code: 0} | from tsc");
    assert_eq!(run.stdout, "[]\n");
    let run = nux(&[], "{stdout: '', stderr: 'ERR_PNPM_NO_SCRIPT Missing script: typecheck', exit_code: 1} | from tsc");
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains("from tsc: this is not what tsc prints"), "{}", run.stdout);
    assert!(run.stdout.contains("Missing script: typecheck"), "{}", run.stdout);
    let run = nux(
        &[],
        &format!("{plain} | lines | each {{|l| $'packages/engine typecheck: ($l)' }} | str join (char nl) | from tsc | each {{|r| $'($r.file):($r.line)' }} | str join ' '"),
    );
    assert_eq!(run.stdout, "packages/engine/a.ts:1 packages/engine/a.ts:3 packages/engine/a.ts:6\n", "{}", run.stdout);
}


/// `py --with` and `js --with`: each key is defined in the code under its name, with its type,
/// next to `data` (the pipeline input), which stays. Without --with, nothing changes; with
/// nothing piped, `data` is null.
#[test]
fn with_defines_values_by_name() {
    let run = nux(
        &[],
        "let limit = 2kb\nlet when = 2026-01-31T10:00:00+01:00\n{\n  py: ([1 2 3] | py --with {limit: $limit, when: $when, n: 2} '[len(data), limit, when.year, n * 2]')\n  js: ([1 2 3] | js --with {limit: $limit, when: $when, n: 2} '[data.length, limit.bytes, when.year, n * 2]')\n}",
    );
    // Python's file size is an int that stays a file size on the way back.
    assert_eq!(run.stdout, "{py:[3,2000b,2026,4],js:[3,2000,2026,4]}\n", "{}{}", run.stdout, run.stderr);
    let run = nux(&[], "{py: (py --with {files: [a b]} '[data, len(files)]'), js: (js --with {files: [a b]} '[data, files.length]')}");
    assert_eq!(run.stdout, "{py:[null,2],js:[null,2]}\n", "{}{}", run.stdout, run.stderr);
    // The pipeline input as a value of its own name.
    let run = nux(&[], "[1 2 3] | js --with {numbers: $in} 'numbers.length'");
    assert_eq!(run.stdout, "3\n", "{}{}", run.stdout, run.stderr);
}

/// Ranges into `py` and `js`: counting down and in steps too (Nushell writes those as
/// `start..next..end`), with or without the end itself.
#[test]
fn ranges_reach_py_and_js_as_numbers() {
    let both = |code: &str| {
        let run = nux(&[], &format!("{{py: ({code} | py 'list(data)'), js: ({code} | js 'data')}}"));
        assert_eq!(run.status, 0, "{code}: {}{}", run.stdout, run.stderr);
        run.stdout.trim_end().to_string()
    };
    assert_eq!(both("1..5"), "{py:[1,2,3,4,5],js:[1,2,3,4,5]}");
    assert_eq!(both("5..1"), "{py:[5,4,3,2,1],js:[5,4,3,2,1]}");
    assert_eq!(both("1..3..9"), "{py:[1,3,5,7,9],js:[1,3,5,7,9]}");
    assert_eq!(both("1..3..<9"), "{py:[1,3,5,7],js:[1,3,5,7]}");
    assert_eq!(both("9..7..<1"), "{py:[9,7,5,3],js:[9,7,5,3]}");
    assert_eq!(both("5..<1"), "{py:[5,4,3,2],js:[5,4,3,2]}");
    // Without an end: a description, with its step.
    let run = nux(&[], "1..3.. | py 'data'");
    assert_eq!(run.stdout, "{start:1,end:null,step:2,inclusive:true}\n", "{}{}", run.stdout, run.stderr);
}

/// `wait-for port`: done as soon as something listens there; it has no time limit of its own,
/// `timeout` stops it. `wait-for` without saying what is an error that says how.
#[test]
fn wait_for_a_port() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let run = nux(&[], &format!("wait-for port {port} | get ready"));
    assert_eq!(run.stdout, "true\n", "{}{}", run.stdout, run.stderr);
    drop(listener);
    let run = nux(&[], &format!("timeout 300ms {{ wait-for port {port} }}"));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("did not finish within 300ms"), "{}", run.stdout);
    let run = nux(&[], &format!("wait-for port {port} --timeout 300ms"));
    assert!(run.stdout.contains("Hint: No command has a time limit of its own: put it in `timeout`"), "{}", run.stdout);
    let run = nux(&[], &format!("wait-for {port}"));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains(&format!("For this: wait-for port {port}")), "{}", run.stdout);
}

/// `wait-for file`: there, with a text in it, or no longer growing; `wait-for pid`: ended.
#[test]
fn wait_for_a_file_and_a_process() {
    let dir = scratch().join(format!("wait-for-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("out.log");
    let writer = {
        let file = file.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::write(&file, "building\n").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(300));
            std::fs::write(&file, "building\nDone in 2s\n").unwrap();
        })
    };
    let f = file.display();
    let run = nux(&[], &format!("timeout 5sec {{ wait-for file '{f}' --contains 'Done in' }} | get size"));
    assert_eq!(run.stdout, "20b\n", "{}{}", run.stdout, run.stderr);
    writer.join().unwrap();
    let run = nux(&[], &format!("timeout 5sec {{ wait-for file '{f}' --stable 300ms }} | get ready"));
    assert_eq!(run.stdout, "true\n", "{}{}", run.stdout, run.stderr);
    let run = nux(&[], &format!("timeout 300ms {{ wait-for file '{f}' --contains nope }}"));
    assert!(run.stdout.contains("did not finish within 300ms"), "{}", run.stdout);
    // A process that ends: done; one that does not: still waiting when the time is up.
    let mut child = Command::new("sleep").arg("0.4").spawn().unwrap();
    let run = nux(&[], &format!("timeout 5sec {{ wait-for pid {} }} | get ready", child.id()));
    assert_eq!(run.stdout, "true\n", "{}{}", run.stdout, run.stderr);
    child.wait().unwrap();
    let mut child = Command::new("sleep").arg("5").spawn().unwrap();
    let run = nux(&[], &format!("timeout 300ms {{ wait-for pid {} }}", child.id()));
    assert!(run.stdout.contains("did not finish within 300ms"), "{}", run.stdout);
    child.kill().unwrap();
    child.wait().unwrap();
}

/// `git show-file`: a file at a revision, with paths from the current directory.
#[test]
fn git_show_file_reads_a_file_at_a_revision() {
    let repo = git_repo("show-file");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/a.txt"), "first\n").unwrap();
    for args in [&["add", "."][..], &["commit", "-q", "-m", "a"]] {
        let status = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@t"]).args(args).current_dir(&repo).status();
        assert!(status.unwrap().success());
    }
    std::fs::write(repo.join("src/a.txt"), "second\n").unwrap();
    assert_eq!(nux_in(&repo, "git show-file HEAD src/a.txt").stdout, "first\n");
    assert_eq!(nux_in(&repo, "git show-file HEAD:src/a.txt").stdout, "first\n");
    assert_eq!(nux_in(&repo.join("src"), "git show-file HEAD a.txt").stdout, "first\n");
    let run = nux_in(&repo, "git show-file HEAD src/nope.txt");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("does not exist in 'HEAD'"), "{}", run.stdout);
}

/// `timeout`: the block's value, input and own error; stopped, with the commands it started,
/// when the time is up.
#[test]
fn timeout_stops_a_block() {
    let run = nux(&[], "[1 2 3] | timeout 2sec { $in | math sum }");
    assert_eq!(run.stdout, "6\n", "{}{}", run.stdout, run.stderr);
    let run = nux(&[], "timeout 2sec { error make {msg: 'its own error'} }");
    assert!(run.stdout.contains("its own error"), "{}", run.stdout);
    let marker = format!("{}.{}", 30 + std::process::id() % 1000, std::process::id());
    let run = nux(&[], &format!("timeout 300ms {{ ^sleep {marker} }}"));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("timeout: the block did not finish within 300ms; it was stopped"), "{}", run.stdout);
    let left = Command::new("pgrep").args(["-f", &format!("sleep {marker}")]).output().unwrap();
    assert!(left.stdout.is_empty(), "sleep {marker} still runs");
}

/// The pnpm module's commands that run pnpm, against a stand-in `pnpm` that prints what pnpm
/// 12 printed for these (no pnpm or registry needed): run-recursive, packages, outdated, why.
/// And without `use nux/pnpm` there are none of them: `pnpm ...` is pnpm.
#[test]
fn pnpm_module_runs_pnpm_and_gives_tables() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch().join(format!("pnpm-module-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::create_dir_all(dir.join("ws/packages/a")).unwrap();
    std::fs::write(dir.join("ws/packages/a/package.json"), r#"{"name":"@t/a","scripts":{"test":"x","build":"y"}}"#).unwrap();
    let fake = dir.join("bin/pnpm");
    std::fs::write(
        &fake,
        r#"#!/bin/sh
case "$*" in
  "--recursive --stream --filter ./packages/a run test --only")
    printf 'Scope: 1 of 2 workspace projects\npackages/a test$ node --test --only\npackages/a test: TAP version 13\npackages/a test: not ok 1 - adds\npackages/a test: 1..1\npackages/a test: Failed\n'
    echo 'ERR_PNPM_RECURSIVE_RUN_FIRST_FAIL  @t/a test' >&2; exit 1 ;;
  "--recursive --stream run nosuch")
    echo 'None of the selected packages has a "nosuch" script' >&2; exit 1 ;;
  "ls --recursive --depth -1 --json")
    printf '[{"name":"root","path":"%s","private":true},{"name":"@t/a","version":"1.0.0","path":"%s/packages/a","private":false}]' "$PWD" "$PWD" ;;
  "outdated --recursive --format json")
    printf '{"left-pad":{"current":"1.0.0","wanted":"1.0.1","latest":"2.0.0","isDeprecated":false,"dependencyType":"dependencies","dependentPackages":[{"name":"@t/a","location":"/x"}]}}'; exit 1 ;;
  "why react --recursive --json")
    printf '[{"name":"react","version":"18.3.1","dependents":[{"name":"ui","version":"1.0.0","dependents":[{"name":"@t/a","version":"1.0.0","depField":"dependencies"}]},{"name":"forms","version":"2.0.0","dependents":[{"name":"ui","version":"1.0.0","deduped":true}]},{"name":"@t/a","version":"1.0.0","depField":"devDependencies"}]}]' ;;
  "--version") echo 12.0.0 ;;
  *) echo "fake pnpm: unexpected: $*" >&2; exit 9 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::path::PathBuf::from(format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap()));
    let run = |code: &str| {
        let run = nux_with(&[], &format!("cd '{}'\nuse nux/pnpm\n{code}", dir.join("ws").display()), &[("PATH", &path)]);
        assert_eq!(run.status, 0, "{code}: {}{}", run.stdout, run.stderr);
        run
    };
    // Options for pnpm after the script, what follows `--` for the script; a failing run is rows.
    let tests = run("pnpm run-recursive test --filter ./packages/a -- --only | select package command status");
    assert_eq!(tests.stdout, "[[package,command,status];[packages/a,\"node --test --only\",failed]]\n");
    assert!(tests.stderr.contains("ERR_PNPM_RECURSIVE_RUN_FIRST_FAIL"), "{}", tests.stderr);
    let failed = nux_with(&[], &format!("cd '{}'\nuse nux/pnpm\npnpm run-recursive nosuch", dir.join("ws").display()), &[("PATH", &path)]);
    assert!(failed.stdout.contains("pnpm failed before any package ran it"), "{}", failed.stdout);
    assert!(failed.stdout.contains("has a \"nosuch\" script"), "{}", failed.stdout);
    assert_eq!(run("pnpm packages | select name path scripts").stdout, "[[name,path,scripts];[root,\".\",[]],[@t/a,packages/a,[test,build]]]\n");
    assert_eq!(
        run("pnpm outdated | select name current latest used_by").stdout,
        "[[name,current,latest,used_by];[left-pad,\"1.0.0\",\"2.0.0\",[@t/a]]]\n"
    );
    // A deduped dependent (`ui` under `forms`) is followed through its dependents elsewhere.
    assert_eq!(
        run("pnpm why react | select package field through").stdout,
        "[[package,field,through];[@t/a,dependencies,[ui]],[@t/a,dependencies,[ui,forms]],[@t/a,devDependencies,[]]]\n"
    );
    // Without `use`, `pnpm ...` is pnpm itself (here the stand-in), also for `pnpm packages`.
    let plain = nux_with(&[], "pnpm --version", &[("PATH", &path)]);
    assert_eq!(plain.stdout, "12.0.0\n", "{}{}", plain.stdout, plain.stderr);
}

/// `pnpm by-package` (with `use nux/pnpm`): what `pnpm -r` printed, per package, without its
/// prefix; and pnpm itself is still there.
#[test]
fn pnpm_by_package_takes_recursive_output_apart() {
    let output = "Scope: 2 of 3 workspace projects\n\
        packages/a test$ node --test\n\
        packages/b test$ node --test\n\
        packages/a test: TAP version 13\n\
        packages/b test: TAP version 13\n\
        packages/a test: ok 1 - adds\n\
        packages/b test: not ok 1 - parses\n\
        packages/b test:   ---\n\
        packages/b test:   error: 'expected 2'\n\
        packages/b test:   ...\n\
        packages/a test: 1..1\n\
        packages/b test: 1..1\n\
        packages/a test: Done\n\
        packages/b test: Failed\n\
        warning x: y\n";
    let code = format!(
        "use nux/pnpm\n\
         let run = {{stdout: {}, stderr: '', exit_code: 1}}\n\
         $run | pnpm by-package | select package status\n\
         $run | pnpm by-package | each {{|p| $p.output | from tap | insert package $p.package }} | flatten | select package status error\n\
         try {{ $run | from tap }} catch {{|e| $e.rendered | str contains 'pnpm by-package' }}",
        serde_json::to_string(output).unwrap()
    );
    let run = nux(&[], &code);
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("[[package,status];[packages/a,done],[packages/b,failed]]"), "{}", run.stdout);
    assert!(run.stdout.contains("[[package,status,error];[packages/a,pass,null],[packages/b,fail,\"expected 2\"]]"), "{}", run.stdout);
    assert!(run.stdout.ends_with("true\n"), "{}", run.stdout);
}

/// A stand-in `npm` that prints what npm 11 printed for these (shapes recorded from the
/// registry, values made small), for the nux/npm tests: no network. Returns its folder and a
/// PATH with it first.
fn fake_npm() -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    // A folder per call: tests run in parallel, and writing a script another test is running
    // fails ("Text file busy", seen on CI).
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = scratch().join(format!("npm-module-{}-{call}", std::process::id()));
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::create_dir_all(dir.join("work")).unwrap();
    let fake = dir.join("bin/npm");
    std::fs::write(
        &fake,
        r#"#!/bin/sh
case "$*" in
  "view left-pad time dist-tags versions --json")
    printf '%s' '{"time":{"created":"2014-03-01T00:00:00.000Z","modified":"2024-01-01T00:00:00.000Z","1.0.0":"2015-01-01T00:00:00.000Z","1.3.0":"2018-04-09T00:00:00.000Z"},"dist-tags":{"latest":"1.3.0","next":"2.0.0-beta.1"},"versions":["1.0.0","1.3.0","2.0.0-beta.1"]}' ;;
  "view left-pad@>=0.0.0-0 || >=2.0.0-0 <2.0.0 version deprecated --json")
    printf '%s' '[{"version":"1.0.0","deprecated":"use String.prototype.padStart()"},{"version":"1.3.0"},{"version":"2.0.0-beta.1"}]' ;;
  "view solo time dist-tags versions --json")
    printf '%s' '{"time":{"created":"2020-01-01T00:00:00.000Z","modified":"2020-01-01T00:00:00.000Z","1.0.0":"2020-01-01T00:00:00.000Z"},"dist-tags":{"latest":"1.0.0"},"versions":"1.0.0"}' ;;
  "view solo@>=0.0.0-0 version deprecated --json")
    printf '%s' '{"version":"1.0.0"}' ;;
  "view fresh time dist-tags versions --json")
    printf '%s' '{"time":{"1.0.0":"2021-01-01T00:00:00.000Z","1.1.0":"2021-02-01T00:00:00.000Z"},"dist-tags":{"latest":"1.1.0"},"versions":["1.0.0","1.1.0"]}' ;;
  "view fresh@>=0.0.0-0 version deprecated --json")
    printf '%s' '["1.0.0","1.1.0"]' ;;
  "view pre time dist-tags versions --json")
    printf '%s' '{"time":{"1.0.0":"2021-01-01T00:00:00.000Z","2.0.0-beta.1":"2021-02-01T00:00:00.000Z","2.0.0":"2021-03-01T00:00:00.000Z"},"dist-tags":{"latest":"2.0.0"},"versions":["1.0.0","2.0.0-beta.1","2.0.0"]}' ;;
  "view pre@>=0.0.0-0 || >=2.0.0-0 <2.0.0 version deprecated --json")
    printf '%s' '[{"version":"1.0.0"},{"version":"2.0.0-beta.1","deprecated":"use 2.0.0"},{"version":"2.0.0"}]' ;;
  "view unordered@6 || 7 --json")
    printf '%s' '[{"name":"unordered","version":"6.14.12"},{"name":"unordered","version":"7.10.14"},{"name":"unordered","version":"6.14.13"}]' ;;
  "view unordered@2 --json")
    printf '%s' '[{"name":"unordered","version":"2.0.0"},{"name":"unordered","version":"2.0.0-beta.10"},{"name":"unordered","version":"2.0.0-beta.9"},{"name":"unordered","version":"2.10.0"},{"name":"unordered","version":"2.9.0"}]' ;;
  "view big time dist-tags versions --json")
    cat "$(dirname "$0")/big-view.json" ;;
  "view big@>=0.0.0-0 version deprecated --json")
    cat "$(dirname "$0")/big-deprecated.json" ;;
  "view left-pad@1 --json")
    printf '%s' '[{"name":"left-pad","version":"1.0.0","_id":"left-pad@1.0.0"},{"name":"left-pad","version":"1.3.0","_id":"left-pad@1.3.0","peerDependencies":{"x":"^1"}}]' ;;
  "view left-pad@1.3.0 --json")
    printf '%s' '{"name":"left-pad","version":"1.3.0","_id":"left-pad@1.3.0","_rev":"1","versions":["1.3.0"],"time":{},"users":{},"readme":"long","main":"index.js"}' ;;
  "view solo@1 --json")
    printf '%s' '{"name":"solo","version":"1.0.0"}' ;;
  "view @t/scoped@2.0.0 --json")
    printf '%s' '{"name":"@t/scoped","version":"2.0.0"}' ;;
  "pack left-pad@1.3.0 --dry-run --json")
    printf '%s' '[{"name":"left-pad","version":"1.3.0","files":[{"path":"index.js","size":1000,"mode":420},{"path":"package.json","size":600,"mode":420}]}]' ;;
  "view nope --json"|"view nope time dist-tags versions --json")
    printf '%s' '{"error":{"code":"E404","summary":"Not Found - GET https://registry.npmjs.org/nope - Not found","detail":"x"}}'
    echo 'npm error code E404' >&2; exit 1 ;;
  "view left-pad@99 --json")
    printf '%s' '{"error":{"code":"E404","summary":"No match found for version 99"}}'
    echo 'npm error code E404' >&2; exit 1 ;;
  "view broken --json")
    echo 'npm error network ECONNREFUSED' >&2; exit 1 ;;
  *) echo "fake npm: unexpected: $*" >&2; exit 9 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    // `big`: 3000 versions, half of them deprecated, as react has (~3000 versions).
    let versions: Vec<String> = (0..3000).map(|n| format!("1.{}.{}", n / 100, n % 100)).collect();
    let times: Vec<String> = versions.iter().enumerate().map(|(n, v)| format!("\"{v}\":\"2020-01-01T00:{:02}:{:02}.000Z\"", n / 60 % 60, n % 60)).collect();
    let view = format!(
        "{{\"time\":{{{}}},\"dist-tags\":{{\"latest\":\"1.29.99\"}},\"versions\":[{}]}}",
        times.join(","),
        versions.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(",")
    );
    std::fs::write(dir.join("bin/big-view.json"), view).unwrap();
    let deprecated: Vec<String> = versions
        .iter()
        .enumerate()
        .map(|(n, v)| if n % 2 == 0 { format!("{{\"version\":\"{v}\",\"deprecated\":\"old\"}}") } else { format!("{{\"version\":\"{v}\"}}") })
        .collect();
    std::fs::write(dir.join("bin/big-deprecated.json"), format!("[{}]", deprecated.join(","))).unwrap();
    let path = std::path::PathBuf::from(format!("{}:{}", dir.join("bin").display(), std::env::var("PATH").unwrap()));
    (dir, path)
}

/// Code after `use nux/npm`, in the stand-in's work folder, with the stand-in on PATH.
fn with_npm(code: &str) -> Run {
    let (dir, path) = fake_npm();
    nux_with(&[], &format!("cd '{}'\nuse nux/npm\n{code}", dir.join("work").display()), &[("PATH", &path)])
}

/// `npm versions`: a row per version, by publish date, with its tags and deprecation; a
/// package of one version (npm: a string, a record) the same.
#[test]
fn npm_versions_gives_a_row_per_version() {
    let run = with_npm("npm versions left-pad | to nuon");
    assert_eq!(
        run.stdout,
        "[[version, published, tags, deprecated]; [\"1.0.0\", 2015-01-01T00:00:00+00:00, [], \"use String.prototype.padStart()\"], [\"1.3.0\", 2018-04-09T00:00:00+00:00, [latest], null], [\"2.0.0-beta.1\", null, [next], null]]\n",
        "{}",
        run.stderr
    );
    let run = with_npm("npm versions solo | to nuon");
    assert_eq!(run.stdout, "[[version, published, tags, deprecated]; [\"1.0.0\", 2020-01-01T00:00:00+00:00, [latest], null]]\n", "{}", run.stderr);
    // No version deprecated: npm gives the versions as plain strings (react, live).
    let run = with_npm("npm versions fresh | get deprecated | to nuon");
    assert_eq!(run.stdout, "[null, null]\n", "{}{}", run.stdout, run.stderr);
    // A deprecated prerelease: `>=0.0.0-0` alone leaves prereleases out (semver), so the
    // query names each prerelease's base too.
    let run = with_npm("npm versions pre | where deprecated != null | get version | to nuon");
    assert_eq!(run.stdout, "[\"2.0.0-beta.1\"]\n", "{}{}", run.stdout, run.stderr);
    // 3000 versions: one lookup per version, not a search through all of them (react took
    // 40 s that way).
    let started = std::time::Instant::now();
    let run = with_npm("npm versions big | where deprecated != null | length");
    assert_eq!(run.stdout, "1500\n", "{}{}", run.stdout, run.stderr);
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "npm versions big took {:?}", started.elapsed());
}

/// `npm info` with a range: the highest version by semver, whatever order npm lists them in
/// (npm keeps the registry's order: a backport comes after a newer line).
#[test]
fn npm_info_takes_the_highest_version_of_a_range() {
    let run = with_npm("npm info 'unordered@6 || 7' | get version");
    assert_eq!(run.stdout, "7.10.14\n", "{}", run.stderr);
    let run = with_npm("npm info unordered@2 | get version");
    assert_eq!(run.stdout, "2.10.0\n", "{}", run.stderr);
}

/// `npm info`: one record; a range is its highest version; without npm's `_` fields and the
/// large ones; a scoped name with a version.
#[test]
fn npm_info_gives_one_record() {
    let run = with_npm("npm info left-pad@1 | get version");
    assert_eq!(run.stdout, "1.3.0\n", "{}", run.stderr);
    let run = with_npm("npm info left-pad@1 | get peerDependencies.x");
    assert_eq!(run.stdout, "^1\n", "{}", run.stderr);
    let run = with_npm("npm info left-pad@1.3.0 | columns | to nuon");
    assert_eq!(run.stdout, "[name, version, main]\n", "{}", run.stderr);
    let run = with_npm("npm info solo@1 | get version");
    assert_eq!(run.stdout, "1.0.0\n", "{}", run.stderr);
    let run = with_npm("npm info @t/scoped@2.0.0 | get name");
    assert_eq!(run.stdout, "@t/scoped\n", "{}", run.stderr);
}

/// `npm files`: what a published package contains, and nothing is downloaded here.
#[test]
fn npm_files_lists_a_published_package() {
    let run = with_npm("npm files left-pad@1.3.0 | to nuon");
    assert_eq!(run.stdout, "[[path, size]; [\"index.js\", 1000b], [\"package.json\", 600b]]\n", "{}", run.stderr);
    let run = with_npm("npm files left-pad@1.3.0 | ignore; glob *.tgz | length");
    assert_eq!(run.stdout, "0\n", "{}", run.stderr);
}

/// What npm says when a package or version is not there, or npm fails without JSON.
#[test]
fn npm_errors_say_what_npm_said() {
    for (code, said) in [
        ("npm info nope", "Not Found - GET https://registry.npmjs.org/nope"),
        ("npm versions nope", "Not Found - GET https://registry.npmjs.org/nope"),
        ("npm info left-pad@99", "No match found for version 99"),
        ("npm info broken", "ECONNREFUSED"),
    ] {
        let run = with_npm(code);
        assert_eq!(run.status, 1, "{code}: {}", run.stdout);
        assert!(run.stdout.contains(said), "{code}: {}", run.stdout);
    }
}

/// nux/npm's tables give every column in every row (AGENTS.md), as
/// `table_commands_give_every_column_in_every_row` checks for the prelude's.
#[test]
fn npm_tables_give_every_column_in_every_row() {
    let run = with_npm(
        "{versions: (npm versions left-pad), files: (npm files left-pad@1.3.0)}
| transpose command rows
| where {|t| ($t.rows | each { columns | str join ',' } | uniq | length) > 1 }
| get command
| to nuon",
    );
    assert_eq!((run.status, run.stdout.as_str()), (0, "[]\n"), "{}{}", run.stdout, run.stderr);
}

/// A URL is up at any answer, also a 404; with --status only at one of those.
#[test]
fn wait_for_a_url_and_a_status() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        }
    });
    let run = nux(&[], &format!("wait-for url http://127.0.0.1:{port}/ | get status"));
    assert_eq!(run.stdout, "404\n", "{}{}", run.stdout, run.stderr);
    let run = nux(&[], &format!("timeout 600ms {{ wait-for url http://127.0.0.1:{port}/ --status [200] }}"));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("did not finish within 600ms"), "{}", run.stdout);
    // Stopped, it says what it last saw, on stderr.
    assert!(run.stderr.contains(&format!("wait-for url http://127.0.0.1:{port}/: stopped after")), "{}", run.stderr);
    assert!(run.stderr.contains("it answered, last with status 404, not one of 200"), "{}", run.stderr);
}

#[test]
fn exit_sets_the_status() {
    assert_eq!(nux(&[], "exit 4").status, 4);
}

#[test]
fn a_large_result_is_saved_to_a_file() {
    let run = nux(&[], "1..5000 | each {|i| {i: $i, s: 'some text'} }");
    assert_eq!(run.status, 0);
    assert!(run.stdout.starts_with("The result is too large to print"), "{}", run.stdout);
    let name = run.stdout.split("kept as `").nth(1).unwrap().split('`').next().unwrap();
    assert!(name.starts_with("result-"), "{}", run.stdout);
    // A next call gets all of it back, as data.
    let back = nux(&[], &format!("kept {name} | where i > 4990 | length"));
    assert_eq!(back.stdout, "10\n", "{}", back.stderr);
    // A preview: the shape, how many rows, and the first ones.
    assert!(run.stdout.contains("table<i: int, s: string>, 5000 rows"), "{}", run.stdout);
    assert!(run.stdout.contains("[[i,s];[1,\"some text\"],[2,\"some text\"]"), "{}", run.stdout);
    // Getting it back whole is too large again: it says the same name, it keeps no copy.
    let again = nux(&[], &format!("kept {name}"));
    assert!(again.stdout.contains(&format!("all of it is kept as `{name}`")), "{}", again.stdout);
    let copies = nux(&[], "kept | where name =~ '^result-' | length").stdout;
    let before = copies.trim().parse::<usize>().unwrap();
    nux(&[], &format!("kept {name}"));
    assert_eq!(nux(&[], "kept | where name =~ '^result-' | length").stdout.trim().parse::<usize>().unwrap(), before);
}

#[test]
fn a_failed_call_keeps_its_variables() {
    let run = nux(&[], "let files = [[name size]; [a 1kb] [b 2kb]]\nlet when = (date now)\nlet n = 4\nopen /nope.json\nlet never = 1");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("Kept from this call: $files (table, 2 rows: name, size), $when (datetime), $n (int)"), "{}", run.stdout);
    assert!(run.stdout.contains("let files = (kept files)"), "{}", run.stdout);
    assert!(!run.stdout.contains("$never"), "{}", run.stdout);
    // Back with its types.
    let back = nux(&[], "kept files | get size | math sum");
    assert_eq!(back.stdout, "3000b\n", "{}", back.stdout);
    assert!(nux(&[], "kept").stdout.contains("files"));
    let missing = nux(&[], "kept fils");
    assert!(missing.stdout.contains("Did you mean files?"), "{}", missing.stdout);
}

/// `$in` (the pipeline input) is not a variable the code made: a failed call does not keep it.
#[test]
fn a_failed_call_does_not_keep_in() {
    let run = nux(&[], "let n = 1\n$in | from json");
    assert_eq!(run.status, 1);
    let kept = run.stdout.lines().find(|l| l.starts_with("Kept from this call:")).unwrap_or_default();
    assert_eq!(kept, "Kept from this call: $n (int). Get one back in a next call: let n = (kept n)", "{}", run.stdout);
}

/// `ast-find`: a pattern without holes, and a hole where the grammar wants a literal (a
/// string after `from`), both work; a broken pattern is still an error, not "no matches".
/// Needs `ast-grep`; skipped without.
#[test]
fn ast_find_patterns() {
    if Command::new("ast-grep").arg("--version").output().is_err() {
        eprintln!("skipped: no ast-grep");
        return;
    }
    let dir = scratch().join(format!("ast-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t.tsx"), "import { a, b } from \"x\";\nimport c from \"y\";\nfoo(1);\n").unwrap();
    let run = nux_in(&dir, "ast-find --lang tsx '\"x\"' t.tsx | select line vars");
    assert_eq!(run.stdout, "[[line,vars];[1,{}]]\n", "{}{}", run.stdout, run.stderr);
    let run = nux_in(&dir, "ast-find --lang tsx 'import { $$$N } from $SRC' t.tsx | get vars.0");
    assert_eq!(run.stdout, "{SRC:r#'\"x\"'#,N:[a,b]}\n", "{}{}", run.stdout, run.stderr);
    let run = nux_in(&dir, "ast-find --lang tsx 'foo(' t.tsx");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("it does not parse as tsx"), "{}", run.stdout);
}

#[test]
fn values_can_be_kept_by_hand() {
    nux(&[], "{a: 1} | kept --save handmade");
    assert_eq!(nux(&[], "kept handmade | get a").stdout, "1\n");
    nux(&[], "kept handmade --drop");
    assert!(nux(&[], "kept handmade").stdout.contains("Nothing is kept as `handmade`"));
}

#[test]
fn try_nu_gives_the_outcome_as_data() {
    let run = nux(&[], "try-nu 'print hello; [1 2] | each { $in * 2 }'");
    assert_eq!(run.stdout, "{value:[2,4],error:null,printed:hello,stderr:\"\"}\n", "{}", run.stderr);
    let run = nux(&[], "try-nu \"let a = 1\nopen /nope.json\" | get error | select code line column");
    assert_eq!(run.stdout, "{code:\"nu::shell::io::file_not_found\",line:2,column:6}\n", "{}", run.stdout);
    // Input is $in; types survive both ways.
    let run = nux(&[], "[1kb 2kb] | try-nu '$in | math sum' | get value");
    assert_eq!(run.stdout, "3000b\n", "{}", run.stdout);
    // A parse error, and a failing external command at the end (a value, not an error).
    assert!(nux(&[], "try-nu 'let x = (' | get error.code").stdout.contains("nu::parser"));
    let run = nux(&[], "try-nu '^sh -c \"exit 3\"' | get value.exit_code");
    assert_eq!(run.stdout, "3\n", "{}", run.stdout);
}

/// A try-nu child must not pass on what made it a child: a nux started inside it, or a nested
/// try-nu without input, would take it for its own.
#[test]
fn try_nu_children_do_not_leak_their_settings() {
    let me = env!("CARGO_BIN_EXE_nux");
    // (The last command's output is in value.stdout, as for any external command at the end.)
    let run = nux(&[], &format!("try-nu '\"1 + 1\" | ^{me}' | get value.stdout"));
    assert_eq!(run.stdout, "2\n", "{}{}", run.stdout, run.stderr);
    // The inner try-nu gets null as input, not the outer [1 2].
    let run = nux(&[], "[1 2] | try-nu 'null | try-nu \"$in\" | get value' | get value | describe");
    assert_eq!(run.stdout, "nothing\n", "{}{}", run.stdout, run.stderr);
}

/// What nux adds after an external command must not move the columns of errors after it.
#[test]
fn errors_after_an_external_command_have_the_right_column() {
    let run = nux(&[], "^true; open /nope.json");
    assert!(run.stdout.contains("line 1, column 13   near `/nope.json`"), "{}", run.stdout);
    let run = nux(&[], "try-nu '^true; ^true; open /nope.json' | get error | select line column");
    assert_eq!(run.stdout, "{line:1,column:20}\n", "{}", run.stdout);
}

/// A folder others can write in could get a prelude of theirs: refused.
#[test]
fn a_nux_dir_others_can_write_in_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch().join(format!("open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
    let run = nux_with(&[], "1", &[("NUX_DIR", dir.as_path())]);
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("others can write in it"), "{}", run.stderr);
    // A folder nux makes itself is owner-only.
    let made = scratch().join(format!("made-{}", std::process::id()));
    assert_eq!(nux_with(&[], "1", &[("NUX_DIR", made.as_path())]).stdout, "1\n");
    assert_eq!(std::fs::metadata(&made).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn try_nu_finds_modules_with_include() {
    let dir = scratch().join(format!("modules-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.nu"), "export def hi [] { 'hi from m' }").unwrap();
    let run = nux(&[], &format!("try-nu -I ['{}'] 'use m.nu; m hi' | get value", dir.display()));
    assert_eq!(run.stdout, "hi from m\n", "{}", run.stdout);
}

#[test]
fn large_text_shows_its_first_and_last_lines() {
    let run = nux(&[], "1..3000 | each {|i| $'line ($i)' } | str join (char nl)");
    assert!(run.stdout.contains("3000 lines"), "{}", run.stdout);
    assert!(run.stdout.contains("line 1\nline 2\n"), "{}", run.stdout);
    assert!(run.stdout.contains("(2980 lines left out)"), "{}", run.stdout);
    assert!(run.stdout.contains("line 3000\nMore, in a next call: kept result-"), "{}", run.stdout);
}

#[test]
fn an_unknown_flag_lists_the_flags_there_are() {
    let run = nux(&[], "search x -q");
    assert!(run.stdout.contains("Hint: The flags of `search`:"), "{}", run.stdout);
    assert!(run.stdout.contains("--context (-C) <int>"), "{}", run.stdout);
    // A single letter of another tool gets no guess.
    assert!(!run.stdout.contains("Did you mean"), "{}", run.stdout);
    // grep's flags that have a way here already: that way, then the flags.
    let run = nux(&[], "search x -c");
    assert!(run.stdout.contains("Hint: `-c`: --files gives one row per file"), "{}", run.stdout);
    assert!(run.stdout.contains("The flags of `search`:"), "{}", run.stdout);
    let run = nux(&[], "search x --files-with-matches");
    assert!(run.stdout.contains("Hint: --files gives one row per file that matches"), "{}", run.stdout);
    let run = nux(&[], "search x --exclude node_modules");
    assert!(run.stdout.contains("--glob '!node_modules/'"), "{}", run.stdout);
    let run = nux(&[], "search x --include '*.ts'");
    assert!(run.stdout.contains("Only some files: --glob '*.ts'"), "{}", run.stdout);
    // grep's -n and -r are taken: they change nothing (they failed every day in the log).
    let here = env!("CARGO_MANIFEST_DIR");
    let run = nux(&[], &format!("search -n -r 'fn other_tool' '{here}/src/assist' | get line | length"));
    assert_eq!((run.status, run.stdout.as_str()), (0, "1\n"), "{}", run.stdout);
    let run = nux(&[], "ls --al");
    assert!(run.stdout.contains("Hint: Did you mean --all?"), "{}", run.stdout);
    // A subcommand: Nushell names it `status`; the hint finds `git status`.
    let run = nux(&[], "git status --branch");
    assert!(run.stdout.contains("The flags of `git status`:"), "{}", run.stdout);
    assert!(run.stdout.contains("For the program `git` itself: `^git status --branch`."), "{}", run.stdout);
    // git's own --short and --porcelain are taken: the table is that form.
    let repo = git_repo("status-short");
    assert_eq!(nux_in(&repo, "git status --short | length").status, 0);
    assert_eq!(nux_in(&repo, "git status --porcelain | length").status, 0);
    // git's --oneline: only short hash and subject.
    let run = nux_in(&repo, "git log --oneline | columns");
    assert_eq!(run.stdout, "[short,subject]\n", "{}{}", run.stdout, run.stderr);
    let run = nux(&[], "git log --pretty=short");
    assert!(run.stdout.contains("nux's git log is a table: --oneline"), "{}", run.stdout);
    assert!(run.stdout.contains("`^git log --pretty=short`"), "{}", run.stdout);
    // A command of a module the same code `use`s: its own flags, not those of Nushell's
    // `http` (which has --full, and so was "Did you mean --full?" for --full).
    let run = nux(&[], "use nux/cookie-jar\ncookie-jar http /tmp/jar get https://example.com --full");
    assert!(run.stdout.contains("The flags of `cookie-jar http`:"), "{}", run.stdout);
    assert!(run.stdout.contains("--no-follow"), "{}", run.stdout);
    assert!(!run.stdout.contains("Did you mean --full"), "{}", run.stdout);
}

/// edit-file says where the file stops matching (and names bash quoting, which is literal
/// text in Nushell); an edit that changes nothing is a warning.
#[test]
fn edit_file_says_where_the_text_differs() {
    let dir = scratch().join(format!("edit-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.js");
    std::fs::write(&file, "function greet(name) {\n    return 'hello ' + name;\n}\nconst it's = 1\n").unwrap();
    let f = file.display();
    let run = nux(&[], &format!("edit-file '{f}' 'return \"hello \" + name' 'x'"));
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("The first 7 characters match, at line 2. Then the file has:"), "{}", run.stdout);
    assert!(run.stdout.contains("'hello ' + name;⏎}"), "{}", run.stdout);
    let run = nux(&[], &format!("edit-file '{f}' \"function greet(name) {{\n  return\" 'x'"));
    assert!(run.stdout.contains("with other whitespace"), "{}", run.stdout);
    let run = nux(&[], &format!("edit-file '{f}' 'const it'\"'\"'s = 1' 'x'"));
    assert!(run.stdout.contains("is bash quoting"), "{}", run.stdout);
    let run = nux(&[], &format!("edit-file '{f}' 'greet' 'greet'"));
    assert_eq!(run.status, 0);
    assert!(run.stderr.contains("edit 1 changes nothing"), "{}", run.stderr);
}

/// edit-file with a `path` column: several files, all or nothing; and --regex.
#[test]
fn edit_file_on_several_files_and_with_a_regex() {
    let dir = scratch().join(format!("edit-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.ts"), "import x from \"./x\";\n").unwrap();
    std::fs::write(dir.join("b.ts"), "import y from \"./x\";\n").unwrap();
    std::fs::write(dir.join("c.ts"), "console.log(1)\nconsole.log(two)\n").unwrap();
    let run = nux_in(&dir, "[[path old new]; [a.ts '\"./x\"' '\"./y\"'] [b.ts '\"./x\"' '\"./y\"']] | edit-file | get replacements");
    assert_eq!(run.stdout, "[1,1]\n", "{}", run.stdout);
    // One edit that does not apply: no file is written.
    let run = nux_in(&dir, "[[path old new]; [a.ts './y' './z'] [b.ts 'nope' 'z']] | edit-file");
    assert_eq!(run.status, 1);
    assert_eq!(std::fs::read_to_string(dir.join("a.ts")).unwrap(), "import x from \"./y\";\n");
    let run = nux_in(&dir, "edit-file c.ts --regex --all 'console\\.log\\((.*)\\)' 'logger.debug($1)' | get replacements");
    assert_eq!(run.stdout, "2\n", "{}", run.stdout);
    assert_eq!(std::fs::read_to_string(dir.join("c.ts")).unwrap(), "logger.debug(1)\nlogger.debug(two)\n");
    // The table of edits as an argument, not piped: agents write it so as often.
    let run = nux_in(&dir, "let edits = [[old new]; ['debug(1)' 'debug(3)'] ['two' 'four']]\nedit-file c.ts $edits | get replacements");
    assert_eq!(run.stdout, "2\n", "{}{}", run.stdout, run.stderr);
    assert_eq!(std::fs::read_to_string(dir.join("c.ts")).unwrap(), "logger.debug(3)\nlogger.debug(four)\n");
    let run = nux_in(&dir, "edit-file c.ts [[old new]; [a b]] 'c'");
    assert!(run.stdout.contains("a table of edits and a `new` as well"), "{}", run.stdout);
    // A number as `old` is still text.
    let run = nux_in(&dir, "edit-file c.ts 3 5 | get replacements");
    assert_eq!(run.stdout, "1\n", "{}{}", run.stdout, run.stderr);
}

/// search: lines before or after, only what matched, grep's text, and globs as paths.
#[test]
fn search_has_grep_s_ways_too() {
    let dir = scratch().join(format!("search-grep-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/a.ts"), "one\nfind me 12\ntwo\nthree\nfour\nfind me 34\nfive\n").unwrap();
    std::fs::write(dir.join("src/b.md"), "find me 56\n").unwrap();
    let run = |code: &str| {
        let run = nux_in(&dir, code);
        assert_eq!(run.status, 0, "{code}: {}{}", run.stdout, run.stderr);
        run.stdout
    };
    assert_eq!(run("search 'find me' src/a.ts --after-context 1 | get line"), "[2,3,6,7]\n");
    assert_eq!(run("search 'find me' src/a.ts --before-context 1 | where not hit | get line"), "[1,5]\n");
    assert_eq!(run("search '\\d+' src/a.ts --only-matching | get text"), "[\"12\",\"34\"]\n");
    assert_eq!(
        run("search 'find me' src/a.ts --after-context 1 | to grep"),
        "src/a.ts:2:find me 12\nsrc/a.ts-3-two\n--\nsrc/a.ts:6:find me 34\nsrc/a.ts-7-five\n"
    );
    assert_eq!(run("search 'find me' src/a.ts | to grep"), "src/a.ts:2:find me 12\nsrc/a.ts:6:find me 34\n");
    // Filtered first, then read: the rows left, with `--` where they no longer follow on.
    assert_eq!(
        run("search 'find me' src/a.ts --context 1 | where line != 6 | to grep"),
        "src/a.ts-1-one\nsrc/a.ts:2:find me 12\nsrc/a.ts-3-two\n--\nsrc/a.ts-5-four\n--\nsrc/a.ts-7-five\n"
    );
    // A text over several lines (an ast-find match): one line each, numbered on.
    assert_eq!(run("[[file line text]; [a.ts 3 \"f(\\n  x)\"]] | to grep"), "a.ts:3:f(\na.ts:4:  x)\n");
    let wrong = nux_in(&dir, "ls | to grep");
    assert!(wrong.stdout.contains("to grep needs the columns file, line and text"), "{}", wrong.stdout);
    // A glob as a path: the files it matches.
    assert_eq!(run("search 'find me' src/*.md | get file"), "[\"src/b.md\"]\n");
    let missing = nux_in(&dir, "search x src/*.rs");
    assert!(missing.stdout.contains("search: nothing matches src/*.rs"), "{}", missing.stdout);
    // Text piped in is searched instead of files.
    assert_eq!(run("\"one\\nfind me\\nthree\" | search 'find' --context 1 | to grep"), "(input)-1-one\n(input):2:find me\n(input)-3-three\n");
}

/// The user's prelude.nu runs in every call; `use home/<name>` and `use project/<name>` find
/// modules in the user's nux folder and in `.nu/` of the nearest folder with a `.git`.
#[test]
fn your_prelude_and_modules() {
    let base = scratch().join(format!("user-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let config = base.join("config");
    let repo = base.join("repo");
    std::fs::create_dir_all(config.join("deep")).unwrap();
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join(".nu")).unwrap();
    std::fs::create_dir_all(repo.join("sub/home")).unwrap();
    std::fs::write(config.join("prelude.nu"), "def hello [] { 'hi from the prelude' }\n$env.FROM_PRELUDE = 'yes'\n").unwrap();
    std::fs::write(config.join("tools.nu"), "export def tool [] { 'a home tool' }\n").unwrap();
    std::fs::write(config.join("deep/mod.nu"), "export def inside [] { 'deep' }\n").unwrap();
    std::fs::write(repo.join(".nu/g.nu"), "export def hello [] { 'the project g' }\n").unwrap();
    // A folder `home` where the call runs does not take the place of the user's modules.
    std::fs::write(repo.join("sub/home/tools.nu"), "export def tool [] { 'the wrong one' }\n").unwrap();
    let run = |code: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nux"))
            .env("NUX_LOG_DIR", base.join("log"))
            .env("NUX_DIR", base.join("nux"))
            .env("NUX_FORMAT", "nuon")
            .env("NUX_CONFIG_DIR", &config)
            .current_dir(repo.join("sub"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(code.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    assert_eq!(run("[(hello) $env.FROM_PRELUDE]"), "[\"hi from the prelude\",yes]\n");
    assert_eq!(run("use home/tools\ntools tool"), "a home tool\n");
    assert_eq!(run("use home/deep\ndeep inside"), "deep\n");
    // An error in such a module names it as it is used.
    std::fs::write(repo.join(".nu/broken.nu"), "export def b [] { nope-command-xyz }\n").unwrap();
    let out = run("use project/broken\nbroken b");
    assert!(out.contains("in project/broken, line 1"), "{out}");
    assert_eq!(run("use project/g\ng hello"), "the project g\n");
    // A prelude that does not load: a warning, and the call still runs.
    std::fs::write(config.join("prelude.nu"), "def broken [ {\n").unwrap();
    let out = run("1 + 1");
    assert!(out.contains("Warning: your prelude or modules did not load") && out.ends_with("2\n"), "{out}");
}

/// `nux help` finds the commands of modules that are loaded only with `use`.
#[test]
fn help_finds_commands_of_modules_to_use() {
    let run = nux(&["help", "cookie-jar", "http"], "");
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("--no-follow"), "{}", run.stdout);
    let run = nux(&["help", "no-such-module-xyz", "x"], "");
    assert_ne!(run.status, 0);
}

#[test]
fn bash_habits_get_the_nushell_way() {
    let hint = |code: &str| {
        let run = nux(&[], code);
        run.stdout.lines().find(|l| l.contains("Hint:")).unwrap_or_default().to_string()
    };
    assert!(hint("echo $(pwd)").contains("`(pwd)`"));
    assert!(hint("export FOO=1").contains("$env.NAME"));
    assert!(hint("ls > out.txt").contains("| save out.txt"));
    assert!(hint("if [ -f x ]; then echo y; fi").contains("path exists"));
    assert!(hint("for f in *.txt; do echo $f; done").contains("glob *.txt"));
    // Nushell explains these itself: no hint of ours.
    assert_eq!(hint("ls && ls"), "");
}

/// Backslashes in strings, several glob patterns, Markdown opened as a table, and `range`.
#[test]
fn common_mistakes_get_the_way_to_write_it() {
    let hint = |code: &str| nux(&[], code).stdout;
    let run = hint("search \"a\\.b\" .");
    assert!(run.contains("`\\.` is none") && run.contains("r#'...'#"), "{run}");
    let run = hint("'it\\'s' | str length");
    assert!(run.contains("'...' has no escapes"), "{run}");
    let run = hint("glob a b");
    assert!(run.contains("`glob` takes one pattern"), "{run}");
    let run = hint("glob --no-dir a b");
    assert!(run.contains("`glob` takes one pattern"), "{run}");

    let dir = scratch().join(format!("md-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.md"), "# Title\n\ntext\n").unwrap();
    let run = hint(&format!("open '{}' | lines", dir.join("notes.md").display()));
    assert!(run.contains("`open --raw notes.md`"), "{run}");
    // Other input type errors: no Markdown hint.
    assert!(!hint("[1 2] | lines").contains("Markdown"));

    // Nushell's own pipelines.
    assert!(hint("[1 2] | do {|r| $r + 1 }").contains("`do` does not give its input"));
    assert!(hint("[[index]; [1] [20]] | where index < 12 or (index > 15)").contains("write `$it.index`"));
    let inside = scratch().join(format!("rm-inside-{}", std::process::id()));
    std::fs::create_dir_all(&inside).unwrap();
    assert!(hint(&format!("cd '{0}'; rm -r '{0}'", inside.display())).contains("`cd` somewhere else first"));
    assert!(hint("\"a\\nb\" | where $it =~ a").contains("That is text, not a list"));

    let run = nux(&[], "[a b c d] | range 1..2");
    assert_eq!((run.status, run.stdout.as_str()), (0, "[b,c]\n"), "{}", run.stderr);
    assert!(run.stderr.contains("write `slice`"), "{}", run.stderr);
}

/// Text in ( ) inside $"..." runs as code: both errors that gives get a hint that says so.
#[test]
fn text_in_parentheses_inside_interpolation_gets_a_hint() {
    let hint = "Inside $\"...\", text in ( ) is code that runs";
    // Balanced: `(a root ...)` runs a command `a`.
    let run = nux(&[], "let t = \"x\"\n$\"Logout in the footer (a root loader says who is admin).($t)\"");
    assert!(run.stdout.contains("Command `a` not found"), "{}", run.stdout);
    assert!(run.stdout.contains(hint), "{}", run.stdout);
    assert!(run.stdout.contains("^git commit -F -"), "{}", run.stdout);
    // An apostrophe in the parentheses: "Unclosed delimiter", on another line.
    let run = nux(&[], "let m = $\"line one\n(it's broken) on line two\"\n$m");
    assert!(run.stdout.contains("nu::parser::unclosed_delimiter"), "{}", run.stdout);
    assert!(run.stdout.contains(hint), "{}", run.stdout);
    // An unknown command outside such a string: no hint of this kind.
    let run = nux(&[], "nonexistent-command-xyz");
    assert!(!run.stdout.contains(hint), "{}", run.stdout);
}

/// A glob pattern Nushell's glob refuses: the hint says which part, where Nushell's own
/// message is only "failed to parse glob expression".
#[test]
fn glob_patterns_that_are_refused_get_a_reason() {
    let run = nux(&[], "glob --no-dir '{,web/,../,/home/coder/}{,.}cloudflare.env'");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("Hint: Nushell's glob has no empty alternatives"), "{}", run.stdout);
    assert!(run.stdout.contains("may not start at `/` or `~`"), "{}", run.stdout);
    let run = nux(&[], "glob '{web,/home/coder}/x'");
    assert!(run.stdout.contains("may not start at `/`"), "{}", run.stdout);
    assert!(!run.stdout.contains("empty alternatives"), "{}", run.stdout);
}

#[test]
fn stopping_nux_ends_the_commands_it_started() {
    let dir = scratch();
    let marker = format!("nux-test-sleep-{}", std::process::id());
    let mut child = Command::new(env!("CARGO_BIN_EXE_nux"))
        .env("NUX_LOG_DIR", dir.join("log"))
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // The marker makes the process findable; sh -c keeps it in its command line.
    child.stdin.take().unwrap().write_all(format!("^sh -c 'sleep 30; echo {marker}'").as_bytes()).unwrap();
    let running = |marker: &str| !Command::new("pgrep").args(["-f", marker]).output().unwrap().stdout.is_empty();
    let started = std::time::Instant::now();
    while !running(&marker) {
        assert!(started.elapsed().as_secs() < 5, "the command never started");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(128 + libc::SIGTERM));
    assert!(!running(&marker), "still running after nux was stopped");
}

/// Plugins are registered and used without `nu`, in a registry of the test's own. Needs the
/// query plugin of Nushell 0.117 at ~/.local/lib/nu-plugins (see the README); skipped without.
#[test]
fn plugins_are_registered_and_used_without_nu() {
    let plugin = PathBuf::from(std::env::var("HOME").unwrap()).join(".local/lib/nu-plugins/nu_plugin_query");
    if !plugin.exists() {
        eprintln!("skipped: no {}", plugin.display());
        return;
    }
    let config_home = scratch().join(format!("xdg-{}", std::process::id()));
    std::fs::create_dir_all(config_home.join("nushell")).unwrap();
    let env = [("XDG_CONFIG_HOME", config_home.as_path())];

    let run = nux_with(&[], "$nu.plugin-path", &env);
    assert_eq!(run.stdout.trim(), config_home.join("nushell/plugin.msgpackz").to_str().unwrap(), "{}", run.stderr);

    let run = nux_with(&[], &format!("plugin add '{}'; plugin list | get name", plugin.display()), &env);
    assert_eq!((run.status, run.stdout.trim()), (0, "[query]"), "{}", run.stderr);

    // A next call finds it in the registry.
    let run = nux_with(&[], "'<p>one</p><p>two</p>' | query web --query p | flatten", &env);
    assert_eq!((run.status, run.stdout.trim()), (0, "[one,two]"), "{}{}", run.stdout, run.stderr);
}

#[test]
fn options_that_are_gone_are_refused() {
    let run = nux(&["-s", "x"], "1");
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("no sessions"));
    assert!(nux(&["-c", "1"], "").stderr.contains("read from stdin"));
    assert!(nux(&["-t", "60"], "1").stderr.contains("no time limit"));
    assert!(nux(&["--json"], "1").stderr.contains("| to json"));
}

#[test]
fn every_call_is_in_the_usage_log() {
    let dir = scratch().join("log");
    nux(&["-r", "testing the log"], "40 + 2");
    let lines: String = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect();
    assert!(lines.contains("\"reason\":\"testing the log\""));
    assert!(lines.contains("\"text\":\"42\""));
}

/// The log has all the caller saw, not only the reply: what `print` printed, the stderr of a
/// command that failed (printed by nux before the error), and what a command wrote itself.
#[test]
fn the_usage_log_has_all_the_caller_saw() {
    let dir = scratch().join(format!("log-seen-{}", std::process::id()));
    let end_of = |code: &str| {
        let _ = std::fs::remove_dir_all(&dir);
        let run = nux_with(&[], code, &[("NUX_LOG_DIR", dir.as_path())]);
        let log: String =
            std::fs::read_dir(&dir).unwrap().map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap()).collect();
        let end = log
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .find(|line| line["event"] == "end")
            .unwrap();
        (run, end)
    };

    let (run, end) = end_of("print 'printed'\nprint -e 'warned'\nfor i in [1] { ^sh -c 'echo by-itself >&2' }\n'reply'");
    assert_eq!(end["text"], "reply");
    assert_eq!(end["stdout"], run.stdout, "stdout is logged as the caller got it");
    assert_eq!(end["stdout"], "printed\nreply\n");
    assert_eq!(end["stderr"], run.stderr);
    assert!(run.stderr.contains("warned") && run.stderr.contains("by-itself"), "{}", run.stderr);

    // The case from the review: git's own message, before nux's "exited with code".
    let repo = dir.with_extension("repo");
    let _ = std::fs::remove_dir_all(&repo);
    let (run, end) = end_of(&format!(
        "^git init -q '{r}'\ncd '{r}'\n^git mv does-not-exist.txt b.txt\nprint never",
        r = repo.display()
    ));
    assert!(end["stdout"].as_str().unwrap().contains("fatal: bad source"), "{end}");
    assert_eq!(end["stdout"], run.stdout);
}

/// `nux-log` reads the log back: `runner` says which nux, and a line cut short (a write
/// that failed halfway) is left out instead of breaking it.
#[test]
fn nux_log_reads_the_log_back() {
    let dir = scratch().join(format!("log-read-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    std::fs::write(
        dir.join(format!("{day}.jsonl")),
        format!(
            "{{\"event\":\"start\",\"id\":\"ts-1\",\"at\":\"{day}T00:00:01.000Z\",\"kind\":\"eval\",\"argv\":[],\"session\":\"s\",\"session_named\":false,\"cwd\":\"/\",\"code\":\"1\",\"reason\":null,\"timeout_seconds\":100,\"json\":false,\"conversation\":null,\"pid\":1}}\n\
             {{\"event\":\"end\",\"id\":\"ts-1\",\"at\":\"{day}T00:00:02.000Z\",\"duration_ms\":5,\"ok\":true,\"outcome\":\"ok\",\"error_code\":null,\"text\":\"1\"}}\n\
             {{\"event\":\"start\",\"id\":\"cut-sh\n"
        ),
    )
    .unwrap();
    let run = nux_with(&["-r", "read back"], "nux-log --since 2day | select runner kind outcome reason", &[("NUX_LOG_DIR", dir.as_path())]);
    assert_eq!(run.status, 0, "{}{}", run.stdout, run.stderr);
    // The TypeScript line (no runner), and this call itself (still running while it reads).
    assert!(run.stdout.contains("[nux-ts,eval,ok,null]"), "{}", run.stdout);
    assert!(run.stdout.contains(&format!("[\"nux-rs {}\",eval,unfinished,\"read back\"]", env!("CARGO_PKG_VERSION"))), "{}", run.stdout);
}


/// CSV and TSV come back as text, as written: Nushell's guess per cell drops the leading zero
/// of a zip code, the `+` of a phone number and the zero of 12.50, and gives one column ints in
/// some rows and floats or text in others. A number is one `into float <column>` away.
#[test]
fn csv_and_tsv_values_stay_text() {
    let run = nux(&[], r#""zip,phone,amount\n02134,+31612345678,12.50\n90210,0612345678,7\n" | from csv | to json --raw"#);
    assert_eq!(
        run.stdout,
        "[{\"zip\":\"02134\",\"phone\":\"+31612345678\",\"amount\":\"12.50\"},{\"zip\":\"90210\",\"phone\":\"0612345678\",\"amount\":\"7\"}]\n",
        "{}",
        run.stderr
    );
    let run = nux(&[], r#""zip\tamount\n02134\t12.50\n" | from tsv | to json --raw"#);
    assert_eq!(run.stdout, "[{\"zip\":\"02134\",\"amount\":\"12.50\"}]\n", "{}", run.stderr);
    // `open` parses with the same commands.
    let run = nux(
        &[],
        r#"let dir = (mktemp -d); "zip,amount\n02134,12.50\n" | save $"($dir)/a.csv"; "zip\tamount\n02134\t12.50\n" | save $"($dir)/a.tsv"
        [(open $"($dir)/a.csv") (open $"($dir)/a.tsv")] | to json --raw"#,
    );
    assert_eq!(run.stdout, "[[{\"zip\":\"02134\",\"amount\":\"12.50\"}],[{\"zip\":\"02134\",\"amount\":\"12.50\"}]]\n", "{}", run.stderr);
    // The flags of Nushell's own still work, --no-infer too (it changes nothing now).
    let run = nux(&[], r##""# note\n01;2\n" | from csv --separator ';' --noheaders --comment '#' --no-infer | to json --raw"##);
    assert_eq!(run.stdout, "[{\"column0\":\"01\",\"column1\":\"2\"}]\n", "{}", run.stderr);
    // nux help says so.
    assert!(nux(&["help"], "").stdout.contains("csv/tsv values are text as written"));
    // To numbers, by column.
    let run = nux(&[], r#""item,amount\na,12.50\nb,7\nc,30\n" | from csv | into float amount | sort-by amount --reverse | get item | to json --raw"#);
    assert_eq!(run.stdout, "[\"c\",\"a\",\"b\"]\n", "{}", run.stderr);
}

/// `ans` (and `in`, `nu`, `env`) are Nushell's own variables: Nushell says so, the hint says
/// what to write instead. Agents wrote `let ans = ...` six times in the eval.
#[test]
fn a_builtin_variable_name_gets_another_name() {
    let run = nux(&[], "let ans = 5; $ans + 1");
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("Hint: `$ans` is Nushell's own; name yours otherwise: `let answer = ...`"), "{}", run.stdout);
    let run = nux(&[], "let in = 5");
    assert!(run.stdout.contains("Hint: `$in` is Nushell's own; name yours otherwise: `let input = ...`"), "{}", run.stdout);
    let run = nux(&[], "[1 2] | each {|nu| $nu }");
    assert!(run.stdout.contains("Hint: `$nu` is Nushell's own"), "{}", run.stdout);
}


/// `into float` and `into int` read numbers as the US writes them: a comma groups thousands
/// (1,500.25), and a comma anywhere else is an error, not a guess. Nushell's own read "1,500"
/// as 1.5 in `into float` and "1,5" as 15 in `into int`.
#[test]
fn into_float_and_int_read_us_numbers() {
    let run = nux(&[], r#"["1,500" "1,500.25" "-12,345,678.5" "12.50" "+1,000"] | into float | to json --raw"#);
    assert_eq!(run.stdout, "[1500.0,1500.25,-12345678.5,12.5,1000.0]\n", "{}", run.stdout);
    let run = nux(&[], r#"["1,500" "-1,234,567" "42"] | into int | to json --raw"#);
    assert_eq!(run.stdout, "[1500,-1234567,42]\n", "{}", run.stdout);
    for (code, says) in [
        (r#"[[item amount]; [a "12.50"] [b "1,5"]] | into float amount"#, r#""1,5" is not a number in US style (1,500.25)"#),
        (r#""1,50.5" | into float"#, r#""1,50.5" is not a number in US style"#),
        (r#""1,5" | into int"#, r#""1,5" is not a whole number in US style (1,500)"#),
    ] {
        let run = nux(&[], code);
        assert_eq!(run.status, 1, "{code}: {}", run.stdout);
        assert!(run.stdout.contains(says), "{code}: {}", run.stdout);
    }
    let run = nux(&[], r#"[[item amount]; [a "12.50"] [b "1,5"]] | into float amount"#);
    assert!(run.stdout.contains("row 1, amount"), "{}", run.stdout);
    assert!(run.stdout.contains("into float-eu"), "{}", run.stdout);
    assert!(nux(&[], r#""1,5" | into int"#).stdout.contains("into int-eu"));
    // Other bases are left alone.
    let run = nux(&[], r#""ff" | into int --radix 16"#);
    assert_eq!(run.stdout, "255\n", "{}", run.stdout);
    // What has no comma converts as before: a table's column, text, a list, a record.
    let run = nux(&[], r#"[[item amount]; [a "12.50"] [b " 7 "] [c 3]] | into float amount | to json --raw"#);
    assert_eq!(run.stdout, "[{\"item\":\"a\",\"amount\":12.5},{\"item\":\"b\",\"amount\":7.0},{\"item\":\"c\",\"amount\":3.0}]\n", "{}", run.stderr);
    let run = nux(&[], r#"["1e3" 2 "-0.5"] | into float | to json --raw"#);
    assert_eq!(run.stdout, "[1000.0,2.0,-0.5]\n", "{}", run.stderr);
    let run = nux(&[], r#"{a: "1.5", b: "x,y"} | into float a | to json --raw"#);
    assert_eq!(run.stdout, "{\"a\":1.5,\"b\":\"x,y\"}\n", "{}", run.stderr);
}

/// A value that is not a number, from a CSV file: the error names the value and its row, and
/// points at the agent's own code, not into nux.
#[test]
fn a_value_that_is_not_a_number_is_named() {
    let run = nux(
        &[],
        r#"let dir = (mktemp -d); "item,amount\na,12.50\nb,7\nc,n/a\n" | save $"($dir)/o.csv"
open $"($dir)/o.csv" | into float amount"#,
    );
    assert_eq!(run.status, 1, "{}", run.stdout);
    assert!(run.stdout.contains(r#""n/a" is not a number"#), "{}", run.stdout);
    assert!(run.stdout.contains("row 2, amount"), "{}", run.stdout);
    assert!(run.stdout.contains("line 2"), "{}", run.stdout);
    assert!(!run.stdout.contains("nux/"), "{}", run.stdout);
    assert!(run.stdout.contains("try { $row.amount | into float } catch { null }"), "{}", run.stdout);
}


/// `into float-eu` and `into int-eu` read numbers as much of Europe writes them: a dot groups
/// thousands, a comma is the decimal one (1.500,25). Anything else is an error that names the
/// US-style command.
#[test]
fn into_float_and_int_eu_read_european_numbers() {
    assert!(nux(&["help"], "").stdout.contains("into float-eu 1.500,25"));
    let run = nux(&[], r#"["1.500,25" "1,5" "1.500" "12" "-1.234.567,5"] | into float-eu | to json --raw"#);
    assert_eq!(run.stdout, "[1500.25,1.5,1500.0,12.0,-1234567.5]\n", "{}", run.stdout);
    let run = nux(&[], r#"["1.500" "1.234.567" "42"] | into int-eu | to json --raw"#);
    assert_eq!(run.stdout, "[1500,1234567,42]\n", "{}", run.stdout);
    let run = nux(&[], r#"[[item amount]; [a "12,50"] [b " 7 "]] | into float-eu amount | to json --raw"#);
    assert_eq!(run.stdout, "[{\"item\":\"a\",\"amount\":12.5},{\"item\":\"b\",\"amount\":7.0}]\n", "{}", run.stdout);
    for (code, says, other) in [
        (r#""1.5" | into float-eu"#, r#""1.5" is not a number in European style (1.500,25)"#, "into float"),
        (r#""1,5" | into int-eu"#, r#""1,5" is not a whole number in European style (1.500)"#, "into int"),
        (r#""n/a" | into float-eu"#, r#""n/a" is not a number"#, "catch { null }"),
    ] {
        let run = nux(&[], code);
        assert_eq!(run.status, 1, "{code}: {}", run.stdout);
        assert!(run.stdout.contains(says), "{code}: {}", run.stdout);
        assert!(run.stdout.contains(other), "{code}: {}", run.stdout);
    }
}
