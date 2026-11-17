//! `open notes.md | lines`: `open` reads a Markdown file as a table of its elements (Nushell's
//! `from md`), not as text, so a command for text refuses it. Agents read Markdown all the
//! time (plans, briefs) and want the text. The table is recognisable by its `content_span`.

use nu_protocol::ShellError;

use super::{ErrorContext, Raw};

pub fn hint(context: &ErrorContext) -> Option<String> {
    let Raw::Shell(ShellError::OnlySupportsThisInputType { wrong_type, .. }) = context.raw else { return None };
    if !(wrong_type.contains("element: string") && wrong_type.contains("content_span")) {
        return None;
    }
    Some("That is a Markdown file as a table: `open` parses .md files. For its text: `open --raw notes.md`.".into())
}
