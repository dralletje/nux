//! Every invocation is recorded, to see afterwards how agents really use it. One file per
//! day (UTC), one JSON object per line, in the same format as the TypeScript nux, so that
//! `nux-log` reads both. Nothing is masked: the code is kept exactly as it was given, the
//! reply exactly as it was printed, and everything else the caller saw on stdout and stderr
//! (only a very long stream loses its middle; see `capture`).
//!
//! An invocation writes two lines with the same `id`: "start" before anything runs, "end"
//! when it is over. A start without an end is an invocation that was killed outright.

use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use serde_json::{Value, json};

use crate::paths::usage_log_dir;

/// How an invocation ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    Ok,
    /// Nushell reported an error (`error_code` says which)
    Error,
    /// The code's last external command exited non-zero
    ExternalFailed,
    /// The command line was wrong
    UsageError,
    /// Nushell could not be set up
    StartFailed,
    /// This process got a signal before it was done (an agent's own time limit)
    CallerKilled,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Error => "error",
            Outcome::ExternalFailed => "external_failed",
            Outcome::UsageError => "usage_error",
            Outcome::StartFailed => "start_failed",
            Outcome::CallerKilled => "caller_killed",
        }
    }
}

pub struct Start<'a> {
    /// eval, try-nu, help, find, invalid
    pub kind: &'a str,
    pub argv: &'a [String],
    pub cwd: &'a str,
    pub code: Option<&'a str>,
    pub reason: Option<&'a str>,
}

struct Open {
    id: String,
    started: Instant,
    ended: bool,
}

static CURRENT: OnceLock<Mutex<Open>> = OnceLock::new();

fn now() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn append(record: Value) {
    let dir = usage_log_dir();
    let path = dir.join(format!("{}.jsonl", chrono::Utc::now().format("%Y-%m-%d")));
    let result = std::fs::create_dir_all(&dir).and_then(|_| {
        // One write per line, in append mode: lines of parallel invocations do not mix.
        let mut file = OpenOptions::new().create(true).append(true).mode(0o600).open(&path)?;
        file.write_all(format!("{record}\n").as_bytes())
    });
    // A log that cannot be written must not break the command it describes.
    if let Err(error) = result {
        eprintln!("nux: could not write the usage log in {}: {error}", dir.display());
    }
}

/// Write the start line. Once per process.
pub fn start(start: Start) {
    let id = uuid::Uuid::new_v4().to_string();
    append(json!({
        "event": "start",
        "id": id,
        "at": now(),
        "kind": start.kind,
        "argv": start.argv,
        "cwd": start.cwd,
        "code": start.code,
        "reason": start.reason,
        "conversation": std::env::var("CLAUDE_CODE_SESSION_ID").ok(),
        "pid": std::process::id(),
        // Which nux wrote this: the TypeScript one has no such field.
        "runner": concat!("nux-rs ", env!("CARGO_PKG_VERSION")),
    }));
    let _ = CURRENT.set(Mutex::new(Open { id, started: Instant::now(), ended: false }));
}

/// Write the end line, once (the first call wins: a signal may race the result).
///
/// `text` is the reply: the result, or the error. `stdout` and `stderr` are everything the
/// caller got on each (`capture`), the reply included: what `print` printed, the `stderr
/// (cmd): ...` lines, what commands wrote themselves. Null when that stream was a terminal.
/// For a call that was stopped, what it had printed until then.
pub fn end(ok: bool, outcome: Outcome, error_code: Option<&str>, text: &str) {
    let Some(current) = CURRENT.get() else { return };
    let Ok(mut open) = current.lock() else { return };
    if open.ended {
        return;
    }
    open.ended = true;
    let (stdout, stderr) = crate::capture::seen();
    append(json!({
        "event": "end",
        "id": open.id,
        "at": now(),
        "duration_ms": open.started.elapsed().as_millis() as u64,
        "ok": ok,
        "outcome": outcome.as_str(),
        "error_code": error_code,
        "text": text,
        "stdout": stdout,
        "stderr": stderr,
    }));
}
