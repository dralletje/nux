//! Ending the processes a call started.
//!
//! Interrupting Nushell stops Nushell's own commands, but not an external command that is
//! running (`^sleep 20`, a hanging test run). When nux gives up on a call (time limit, or
//! nux itself is told to stop), it ends everything it started, so that nothing is left
//! running without anyone waiting for it.

use std::{collections::HashMap, time::Duration};

/// The parent of every process, from /proc (Linux).
fn parents() -> HashMap<i32, i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return HashMap::new() };
    entries
        .filter_map(|entry| {
            let pid: i32 = entry.ok()?.file_name().to_str()?.parse().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            // "pid (name) state ppid ...": the name may contain spaces and parentheses.
            let after_name = &stat[stat.rfind(')')? + 2..];
            let ppid = after_name.split(' ').nth(1)?.parse().ok()?;
            Some((pid, ppid))
        })
        .collect()
}

/// Every process below this one: children, their children, and so on.
fn descendants() -> Vec<i32> {
    let parents = parents();
    let mut found = vec![std::process::id() as i32];
    let mut index = 0;
    while index < found.len() {
        let parent = found[index];
        found.extend(parents.iter().filter(|(_, ppid)| **ppid == parent).map(|(pid, _)| *pid));
        index += 1;
    }
    found.remove(0);
    found
}

/// Ask every process below this one to stop (SIGTERM); what is still there after a moment
/// is killed (SIGKILL).
pub fn end_all() {
    let first = descendants();
    if first.is_empty() {
        return;
    }
    for pid in &first {
        unsafe { libc::kill(*pid, libc::SIGTERM) };
    }
    std::thread::sleep(Duration::from_millis(300));
    for pid in descendants() {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
}
