//! What the caller got to see, for the usage log: everything written to nux's stdout and
//! stderr. Not only the reply: `print`, the `stderr (cmd): ...` lines, warnings, and what
//! external commands write straight to the terminal (inside a loop, say) go there too, and
//! none of it passes through the reply.
//!
//! So file descriptors 1 and 2 are pipes, each read by a thread that passes everything on to
//! where it went before and keeps a copy. External commands inherit the pipes, so their
//! output is seen too. A descriptor that is a terminal is left alone (a person, not an agent).

use std::{
    collections::VecDeque,
    io::{IsTerminal, Read, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread::JoinHandle,
    time::Duration,
};

/// How much of a stream the log keeps: this much of its start, and this much of its end.
const KEEP: usize = 64 * 1024;

/// What is kept of one stream: its start, its end, and how much fell in between.
#[derive(Default)]
struct Kept {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    dropped: usize,
    /// The pipe is closed: nobody writes to it any more.
    closed: bool,
}

impl Kept {
    fn add(&mut self, bytes: &[u8]) {
        let into_head = bytes.len().min(KEEP - self.head.len());
        self.head.extend_from_slice(&bytes[..into_head]);
        self.tail.extend(&bytes[into_head..]);
        if self.tail.len() > KEEP {
            let over = self.tail.len() - KEEP;
            self.tail.drain(..over);
            self.dropped += over;
        }
    }

    fn text(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.head).into_owned();
        if self.dropped > 0 {
            text.push_str(&format!("\n[… {} bytes not logged …]\n", self.dropped));
        }
        let (a, b) = self.tail.as_slices();
        text.push_str(&String::from_utf8_lossy(&[a, b].concat()));
        text
    }
}

struct Stream {
    fd: RawFd,
    /// Where the stream went before: the caller's pipe.
    original: OwnedFd,
    kept: Arc<(Mutex<Kept>, Condvar)>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

static STREAMS: OnceLock<Vec<Stream>> = OnceLock::new();

/// Put pipes in place of stdout and stderr (those that are not terminals). Once, at the start,
/// before anything is written.
pub fn start() {
    let mut streams = Vec::new();
    for (fd, is_terminal) in [(1, std::io::stdout().is_terminal()), (2, std::io::stderr().is_terminal())] {
        if is_terminal {
            continue;
        }
        if let Some(stream) = capture(fd) {
            streams.push(stream);
        }
    }
    let _ = STREAMS.set(streams);
}

fn capture(fd: RawFd) -> Option<Stream> {
    // A copy of where the stream goes now, not inherited by the commands the code starts.
    let original = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if original < 0 {
        return None;
    }
    let original = unsafe { OwnedFd::from_raw_fd(original) };
    let (mut reader, writer) = std::io::pipe().ok()?;
    if unsafe { libc::dup2(writer.as_raw_fd(), fd) } < 0 {
        return None;
    }
    // From here the pipe's only writer in this process is `fd` itself.
    drop(writer);
    let kept: Arc<(Mutex<Kept>, Condvar)> = Arc::default();
    let thread = {
        let kept = kept.clone();
        let mut out = std::fs::File::from(original.try_clone().ok()?);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                let n = match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                // Passed on first: the caller must not wait for the log.
                let _ = out.write_all(&buffer[..n]);
                if let Ok(mut kept) = kept.0.lock() {
                    kept.add(&buffer[..n]);
                }
            }
            if let Ok(mut kept) = kept.0.lock() {
                kept.closed = true;
            }
            kept.1.notify_all();
        })
    };
    Some(Stream { fd, original, kept, thread: Mutex::new(Some(thread)) })
}

/// What was written so far to stdout and stderr (null for one that is not captured).
pub fn seen() -> (Option<String>, Option<String>) {
    let text = |fd| {
        STREAMS.get()?.iter().find(|s| s.fd == fd).map(|s| s.kept.0.lock().map(|k| k.text()).unwrap_or_default())
    };
    (text(1), text(2))
}

/// The end of the call: everything nux wrote is handed over, and the streams go back to where
/// they were. Waits a moment for the pipes to close, so that what was written last is in
/// [`seen`]; a command the code left running (a server started with `&`) may keep one open.
pub fn stop() {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    let Some(streams) = STREAMS.get() else { return };
    for stream in streams {
        // Closes nux's end of the pipe: the reader sees the end once nobody else writes.
        unsafe { libc::dup2(stream.original.as_raw_fd(), stream.fd) };
    }
    for stream in streams {
        let (lock, closed) = &*stream.kept;
        if let Ok(kept) = lock.lock() {
            let _ = closed.wait_timeout_while(kept, Duration::from_millis(200), |k| !k.closed);
        }
    }
}

/// After [`stop`]: wait until every pipe is closed, so that a command still running (and
/// writing) has its output passed on, as it was before these pipes.
pub fn drain() {
    let Some(streams) = STREAMS.get() else { return };
    for stream in streams {
        if let Some(thread) = stream.thread.lock().ok().and_then(|mut t| t.take()) {
            let _ = thread.join();
        }
    }
}
