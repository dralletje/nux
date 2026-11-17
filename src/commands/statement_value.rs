//! Every value a call makes is shown, not only the last one. Nushell drops the value of a
//! statement that is not the last of a block (`open package.json` on one line, `ls` on the
//! next: only the `ls` is seen). nux adds this command after each such statement at the top
//! level of the code (src/rewrite.rs): it prints the value under a line that says which
//! statement it is, and gives nothing on, so the code runs as it did.
//!
//! When anything was shown this way, the call's last value gets such a line too
//! (src/engine.rs asks [`shown`]), so that every value has one.

use std::{
    io::Write,
    sync::atomic::{AtomicUsize, Ordering},
};

use nu_engine::command_prelude::*;

/// The name nux adds to the code. Not meant to be written by hand.
pub const NAME: &str = "nux-statement-value";

static SHOWN: AtomicUsize = AtomicUsize::new(0);

/// How many values of earlier statements were shown in this call.
pub fn shown() -> usize {
    SHOWN.load(Ordering::Relaxed)
}

/// The line above a statement's value: where it is in the code, and what it is.
pub fn header(line: usize, statement: &str) -> String {
    format!("── line {line}: {statement}")
}

#[derive(Clone)]
pub struct StatementValue;

impl Command for StatementValue {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Added by nux after statements at the top level: shows a statement's value, which Nushell would drop."
    }

    fn signature(&self) -> Signature {
        Signature::build(NAME)
            .input_output_types(vec![(Type::Any, Type::Nothing)])
            .required("line", SyntaxShape::Int, "The line the statement starts on.")
            .required("statement", SyntaxShape::String, "The statement, as shown above its value.")
            .category(Category::Custom("nux".into()))
    }

    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let line: i64 = call.req(engine_state, stack, 0)?;
        let statement: String = call.req(engine_state, stack, 1)?;
        let value = input.into_value(call.head)?;
        // No value, or text with nothing in it (`^git add` printed nothing): nothing to show.
        let empty = match &value {
            Value::Nothing { .. } => true,
            Value::String { val, .. } => val.trim().is_empty(),
            _ => false,
        };
        if !empty {
            let text = crate::engine::value_text(engine_state, &value);
            SHOWN.fetch_add(1, Ordering::Relaxed);
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{}\n{text}", header(line.max(0) as usize, &statement));
            let _ = out.flush();
        }
        Ok(PipelineData::empty())
    }
}
