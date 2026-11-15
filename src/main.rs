//! nux: Nushell for agents. Code on stdin, one fresh Nushell per call, results as data.

mod args;
mod assist;
mod children;
mod commands;
mod engine;
mod format;
mod paths;
mod prelude;
mod rewrite;
mod usage_log;

use std::{
    io::{IsTerminal, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use args::{Arguments, CODE_ON_STDIN, NO_SESSIONS, UsageError};
use engine::{Nu, Reply};
use usage_log::Outcome;

fn help_text() -> String {
    include_str!("help.txt")
        .replace("{log_dir}", &paths::usage_log_dir().to_string_lossy())
        .replace("{kept_dir}", &commands::kept::dir().to_string_lossy())
}

/// Set when nux is being stopped: the signal decides how the call ends, not the code, which
/// only fails because its commands are being ended.
static STOPPING: AtomicBool = AtomicBool::new(false);

/// Print the reply, record how the call ended, and exit.
fn finish(reply: Reply) -> ! {
    while STOPPING.load(Ordering::Relaxed) {
        std::thread::park();
    }
    usage_log::end(reply.ok, reply.outcome, reply.error_code.as_deref(), &reply.text);
    let mut out = std::io::stdout().lock();
    if !reply.text.is_empty() {
        let _ = writeln!(out, "{}", reply.text);
    }
    let _ = out.flush();
    std::process::exit(reply.exit_code)
}

fn fail(outcome: Outcome, message: impl Into<String>) -> ! {
    let message = message.into();
    usage_log::end(false, outcome, None, &message);
    eprintln!("{message}");
    std::process::exit(1)
}

fn read_stdin() -> String {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return String::new();
    }
    let mut code = String::new();
    let _ = stdin.read_to_string(&mut code);
    code
}

/// Stopped before the reply (an agent's own time limit does this): end what the code started
/// and say so in the usage log. (A SIGKILL cannot be caught: then the code's external
/// commands keep running.)
fn record_signals(interrupt: Arc<AtomicBool>) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGHUP, SIGINT, SIGTERM]) else { return };
    std::thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            STOPPING.store(true, Ordering::Relaxed);
            let name = match signal {
                SIGHUP => "SIGHUP",
                SIGINT => "SIGINT",
                _ => "SIGTERM",
            };
            usage_log::end(false, Outcome::CallerKilled, None, &format!("received {name}"));
            interrupt.store(true, Ordering::Relaxed);
            children::end_all();
            std::process::exit(128 + signal);
        }
    });
}

fn main() {
    // First, while nux has one thread (it removes environment variables).
    let try_nu_child = commands::try_nu::take_child_settings();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| "/".into());
    let cwd_text = cwd.to_string_lossy().into_owned();

    let arguments = match args::parse(&argv) {
        Ok(arguments) => arguments,
        Err(UsageError(message)) => {
            // Code that came with a rejected command line is still worth keeping. (After -c
            // it is in `argv`; stdin is left alone then, nobody may close it.)
            let code = if argv.iter().any(|a| a == "-c" || a == "--code") { String::new() } else { read_stdin() };
            usage_log::start(usage_log::Start {
                kind: "invalid",
                argv: &argv,
                cwd: &cwd_text,
                code: (!code.is_empty()).then_some(code.as_str()),
                reason: None,
            });
            fail(Outcome::UsageError, message);
        }
    };
    let Arguments { reason, help, words } = arguments;
    let (command, words) = match words.split_first() {
        Some((command, rest)) => (Some(command.as_str()), rest.to_vec()),
        None => (None, Vec::new()),
    };

    // Code is read before anything else happens, so that the usage log has it whatever follows.
    let runs_code = command.is_none() && !help;
    let code = if runs_code { read_stdin() } else { String::new() };
    let kind = match command {
        None if help || (code.trim().is_empty() && std::io::stdin().is_terminal()) => "help",
        None => "eval",
        Some(c @ ("help" | "find" | "guide")) => c,
        Some(_) => "invalid",
    };
    let kind = if kind == "eval" && try_nu_child.is_some() { "try-nu" } else { kind };
    usage_log::start(usage_log::Start {
        kind,
        argv: &argv,
        cwd: &cwd_text,
        code: (runs_code && !code.is_empty()).then_some(code.as_str()),
        reason: reason.as_deref(),
    });

    let interrupt = Arc::new(AtomicBool::new(false));
    record_signals(interrupt.clone());

    match command {
        Some("help") if words.is_empty() => finish(Reply::ok(help_text())),
        None if help => finish(Reply::ok(help_text())),
        Some("guide") => finish(Reply::ok(include_str!("guide.md").trim())),
        Some("stop" | "list") => fail(Outcome::UsageError, NO_SESSIONS),
        Some("help" | "find") => {}
        Some(other) => fail(Outcome::UsageError, format!("Unknown command \"{other}\". {CODE_ON_STDIN}\nHelp: nux help")),
        None if code.trim().is_empty() => {
            if std::io::stdin().is_terminal() {
                finish(Reply::ok(help_text()));
            }
            fail(Outcome::UsageError, format!("No code given. {CODE_ON_STDIN}\nHelp: nux help"));
        }
        None => {}
    }
    if command == Some("find") && words.is_empty() {
        fail(Outcome::UsageError, "Usage: nux find <word>");
    }

    if let Err(problem) = paths::check_nux_dir() {
        fail(Outcome::StartFailed, problem);
    }
    let include = try_nu_child.as_ref().and_then(|child| child.include.clone());
    let mut nu = match Nu::start(&cwd, interrupt, include) {
        Ok(nu) => nu,
        Err(message) => fail(Outcome::StartFailed, message),
    };
    if let Some(child) = try_nu_child {
        run_as_try_nu_child(&mut nu, &code, child);
    }
    let mut reply = match command {
        Some("help") => nu.help(&words),
        Some("find") => nu.find(&words.join(" ")),
        _ => nu.run_code(&code),
    };
    if let Some(problem) = &nu.prelude_problem {
        let warning = format!("Warning: nux's own commands (py, js, search, git, ...) are missing, because the prelude did not load:\n{problem}");
        reply.text = format::join(&[&warning, &reply.text]);
    }
    finish(reply)
}

/// A child of `try-nu`: write `{value, error}` to the file it named, as NUON, and print only
/// what the code itself prints.
fn run_as_try_nu_child(nu: &mut Nu, code: &str, child: commands::try_nu::ChildSettings) -> ! {
    let result_file = child.result;
    let input = child
        .input
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| nuon::from_nuon(&text, None).ok())
        .unwrap_or_else(|| nu_protocol::Value::nothing(nu_protocol::Span::unknown()));
    let result = nu.run_for_result(code, input);
    let failed = !matches!(result.as_record().ok().and_then(|r| r.get("error")), Some(nu_protocol::Value::Nothing { .. }));
    let text = nu.to_nuon(&result);
    let written = std::fs::write(&result_file, &text);
    usage_log::end(!failed, if failed { Outcome::Error } else { Outcome::Ok }, None, &text);
    let _ = std::io::stdout().flush();
    if let Err(error) = written {
        eprintln!("nux: could not write the result of try-nu to {}: {error}", result_file.display());
        std::process::exit(2);
    }
    std::process::exit(if failed { 1 } else { 0 })
}
