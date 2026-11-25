//! `from csv` and `from tsv` give text, as written. Nushell's own guess a type for each cell
//! on its own: 02134 became 2134, +31612345678 lost its +, 12.50 became 12.5, and one column
//! could hold ints in some rows and floats or text in others, without a word. Here a value is
//! always text (Nushell's --no-infer), and `into float <column>` makes numbers when asked.
//!
//! These take the place of Nushell's commands under the same names, so `open x.csv` parses
//! with them too. Each runs Nushell's own with the agent's flags and --no-infer added, under
//! the agent's call, so values (and errors about them) point at the agent's code.

use nu_engine::command_prelude::*;
use nu_protocol::{
    ast::{self, Expr, Expression},
    engine::CallImpl,
};

#[derive(Clone)]
pub struct FromCsvText;

#[derive(Clone)]
pub struct FromTsvText;

const ADDED: &str = "\n\nIn nux every value stays text, as written: 02134 stays \"02134\". For numbers, convert \
                     the columns that hold them: `into float amount price`, `into int count`. --no-infer is \
                     accepted and changes nothing.";

impl Command for FromCsvText {
    fn name(&self) -> &str {
        "from csv"
    }
    fn description(&self) -> &str {
        "Parse text as .csv and create table. Every value stays text, as written."
    }
    fn extra_description(&self) -> &str {
        ADDED.trim_start()
    }
    fn signature(&self) -> Signature {
        nu_command::FromCsv.signature()
    }
    fn search_terms(&self) -> Vec<&str> {
        vec!["csv", "comma separated", "spreadsheet"]
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                description: "Text as written, then a column as numbers",
                example: "\"item,amount\\na,12.50\\nb,7\\n\" | from csv | into float amount | sort-by amount",
                result: None,
            },
            Example { description: "Another separator, no header row", example: "open --raw data.txt | from csv --separator ';' --noheaders", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let text_call = as_text(engine_state, stack, call, &["separator", "comment", "quote", "escape", "trim"])?;
        nu_command::FromCsv.run(engine_state, stack, &text_call, input)
    }
}

impl Command for FromTsvText {
    fn name(&self) -> &str {
        "from tsv"
    }
    fn description(&self) -> &str {
        "Parse text as .tsv and create table. Every value stays text, as written."
    }
    fn extra_description(&self) -> &str {
        ADDED.trim_start()
    }
    fn signature(&self) -> Signature {
        nu_command::FromTsv.signature()
    }
    fn search_terms(&self) -> Vec<&str> {
        vec!["tsv", "tab separated"]
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            description: "Text as written, then a column as numbers",
            example: "\"item\\tamount\\na\\t12.50\\n\" | from tsv | into float amount",
            result: None,
        }]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let text_call = as_text(engine_state, stack, call, &["comment", "quote", "escape", "trim"])?;
        nu_command::FromTsv.run(engine_state, stack, &text_call, input)
    }
}

/// The agent's call again, its flags evaluated, with --no-infer: what Nushell's own command
/// reads its flags from. `strings` are the flags that take a text; the rest are switches.
fn as_text(engine_state: &EngineState, stack: &mut Stack, call: &Call, strings: &[&str]) -> Result<Call<'static>, ShellError> {
    let mut inner = ast::Call::new(call.head);
    let named = |name: &str| (Spanned { item: name.to_string(), span: call.head }, None, None);
    for name in strings {
        if let Some(value) = call.get_flag::<Spanned<String>>(engine_state, stack, name)? {
            let text = Expression::new_unknown(Expr::String(value.item), value.span, Type::String);
            inner.add_named((Spanned { item: name.to_string(), span: call.head }, None, Some(text)));
        }
    }
    for name in ["noheaders", "flexible"] {
        if call.has_flag(engine_state, stack, name)? {
            inner.add_named(named(name));
        }
    }
    inner.add_named(named("no-infer"));
    Ok(Call { head: call.head, decl_id: call.decl_id, inner: CallImpl::AstBox(Box::new(inner)) })
}
