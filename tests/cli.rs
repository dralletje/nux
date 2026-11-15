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
        .envs(env.iter().map(|(k, v)| (*k, *v)))
        .env("NUX_LOG_DIR", dir.join("log"))
        .env("NUX_DIR", dir.join("nux"))
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
fn an_earlier_command_that_succeeds_shows_only_its_stderr() {
    let run = nux(&[], "^sh -c 'echo hidden; echo seen >&2'\n42");
    assert_eq!((run.status, run.stdout.as_str()), (0, "stderr (^sh -c 'echo hidden; echo seen >&2'): seen\n42\n"));
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
    // `py` needs its runner as a file: written out from the binary into NUX_DIR.
    let run = nux(&[], "[1 2 3] | py 'sum(data)'");
    assert_eq!((run.status, run.stdout.as_str()), (0, "6\n"), "{}", run.stderr);
    let folders: Vec<_> = std::fs::read_dir(scratch().join("nux/prelude")).unwrap().filter_map(Result::ok).collect();
    assert_eq!(folders.len(), 1, "one folder, named after the prelude's hash");
    assert!(folders[0].path().join("py_runner.py").exists());
    assert!(folders[0].path().join("browser/mod.nu").exists());

    // Part of it removed (a temp cleanup), or cut short: written again on the next call.
    let dir = folders[0].path();
    std::fs::remove_file(dir.join("py_runner.py")).unwrap();
    std::fs::remove_dir_all(dir.join("browser")).unwrap();
    std::fs::write(dir.join("nuon.py"), "").unwrap();
    let run = nux(&[], "[1 2 3] | py 'sum(data)'");
    assert_eq!((run.status, run.stdout.as_str()), (0, "6\n"), "{}{}", run.stdout, run.stderr);
    assert!(dir.join("browser/mod.nu").exists());
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
    let run = nux(&[], "search x -A 2");
    assert!(run.stdout.contains("Hint: The flags of `search`:"), "{}", run.stdout);
    assert!(run.stdout.contains("--context (-C) <int>"), "{}", run.stdout);
    // A single letter of another tool gets no guess.
    assert!(!run.stdout.contains("Did you mean"), "{}", run.stdout);
    let run = nux(&[], "ls --al");
    assert!(run.stdout.contains("Hint: Did you mean --all?"), "{}", run.stdout);
    // A subcommand: Nushell names it `status`; the hint finds `git status`.
    let run = nux(&[], "git status --short");
    assert!(run.stdout.contains("Hint: `git status` has no flags."), "{}", run.stdout);
    assert!(run.stdout.contains("For the program `git` itself: `^git status --short`."), "{}", run.stdout);
    let run = nux(&[], "git log --oneline");
    assert!(run.stdout.contains("The flags of `git log`:"), "{}", run.stdout);
    assert!(run.stdout.contains("`^git log --oneline`"), "{}", run.stdout);
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
