//! Help for the agent reading nux's output, beyond what Nushell itself says.
//!
//! Everything of this kind lives in this folder. The engine calls the hooks below and knows
//! nothing about the helpers behind them:
//!
//! - [`error_hints`]: after an error, lines that say how to fix it. Each helper looks at the
//!   error (and the code around it) and gives a hint or nothing. To add one: a file with a
//!   `pub fn hint(&ErrorContext) -> Option<String>`, and its name in [`ERROR_HELPERS`].
//! - [`large_value`] / [`large_text`]: what to show of a result too large to print (all of
//!   it is kept by `commands/kept.rs`; these only say what it looks like).
//!
//! A helper should only speak when it is sure; a wrong hint costs more than no hint.

mod flags;
mod preview;
mod shell;

use miette::Diagnostic;
use nu_protocol::{CompileError, ParseError, ShellError, engine::EngineState};

use crate::format::{CODE_FILE, Sources};

pub use preview::{large_text, large_value};

/// The helpers that give hints for errors, asked in this order.
const ERROR_HELPERS: &[fn(&ErrorContext) -> Option<String>] = &[flags::hint, shell::hint];

/// The error as Nushell made it, for helpers that need its fields.
pub enum Raw<'a> {
    Parse(&'a ParseError),
    Compile(&'a CompileError),
    Shell(&'a ShellError),
}

impl Raw<'_> {
    fn diagnostic(&self) -> &dyn Diagnostic {
        match self {
            Raw::Parse(e) => *e,
            Raw::Compile(e) => *e,
            Raw::Shell(e) => *e,
        }
    }
}

/// What a helper gets to look at.
pub struct ErrorContext<'a> {
    pub raw: &'a Raw<'a>,
    /// Nushell's error code, e.g. `nu::parser::unknown_flag`.
    pub code: Option<String>,
    /// The text of the agent's code the error points at (its first label), when it points there.
    pub near: Option<String>,
    /// The whole line of the agent's code that contains it.
    pub line: Option<String>,
    /// For looking up commands and their signatures.
    pub engine: &'a EngineState,
}

/// Hints for an error, as lines to print under it (empty when no helper has one).
pub fn error_hints(raw: &Raw, sources: &dyn Sources, engine: &EngineState) -> String {
    let error = raw.diagnostic();
    let label = error.labels().and_then(|mut labels| labels.next());
    let (near, line) = match label.as_ref().and_then(|label| sources.file_of(label.offset()).map(|file| (label, file))) {
        Some((label, file)) if file.name == CODE_FILE => {
            let at = (label.offset() - file.start).min(file.content.len());
            let end = (at + label.len()).min(file.content.len());
            let line_start = file.content[..at].iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
            let line_end = file.content[at..].iter().position(|b| *b == b'\n').map_or(file.content.len(), |i| at + i);
            (
                Some(String::from_utf8_lossy(&file.content[at..end]).into_owned()),
                Some(String::from_utf8_lossy(&file.content[line_start..line_end]).into_owned()),
            )
        }
        _ => (None, None),
    };
    let context = ErrorContext { raw, code: error.code().map(|c| c.to_string()), near, line, engine };
    ERROR_HELPERS
        .iter()
        .filter_map(|helper| helper(&context))
        .map(|hint| {
            let mut lines = hint.lines();
            let first = format!("  Hint: {}", lines.next().unwrap_or_default());
            std::iter::once(first).chain(lines.map(|l| format!("    {l}"))).collect::<Vec<_>>().join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
