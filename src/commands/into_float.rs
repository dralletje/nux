//! `into float`, which does not guess. Nushell's own reads a comma as a decimal comma, so
//! "1,500" became 1.5 where it was as likely fifteen hundred; only the agent knows which, so
//! here a comma is an error that says how to say it. And a value that is not a number is
//! named in the error, with its row and column: Nushell's says "can't convert string to
//! float" near where the table came from, which for a file of a thousand rows says little.
//!
//! The values are checked first; then Nushell's own converts them, as before.

use nu_engine::command_prelude::*;
use nu_protocol::{ast::CellPath, shell_error::generic::GenericError};

#[derive(Clone)]
pub struct IntoFloat;

impl Command for IntoFloat {
    fn name(&self) -> &str {
        "into float"
    }
    fn description(&self) -> &str {
        nu_command::IntoFloat.description()
    }
    fn extra_description(&self) -> &str {
        "In nux a text with a comma is refused, not guessed at (\"1,500\": thousands, or a decimal comma?): \
         remove it first, or make it a dot. A text that is not a number is named in the error, with its row."
    }
    fn signature(&self) -> Signature {
        nu_command::IntoFloat.signature()
    }
    fn search_terms(&self) -> Vec<&str> {
        nu_command::IntoFloat.search_terms()
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "Convert a column of a table", example: "[[num]; ['5.01']] | into float num", result: None },
            Example {
                description: "Thousands separated by commas: remove them first",
                example: "[[num]; ['1,500']] | update num { str replace --all ',' '' } | into float num",
                result: None,
            },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let paths: Vec<CellPath> = call.rest(engine_state, stack, 0)?;
        let mut input = input;
        let metadata = input.take_metadata();
        let value = input.into_value(call.head)?;
        match &value {
            Value::List { vals, .. } => {
                for (row, item) in vals.iter().enumerate() {
                    check_at(item, &paths, Some(row))?;
                }
            }
            other => check_at(other, &paths, None)?,
        }
        nu_command::IntoFloat.run(engine_state, stack, call, PipelineData::value(value, metadata))
    }
}

/// Check the cells of one row (or of a value that is not in a list) at the given paths, or
/// the value itself without paths.
fn check_at(value: &Value, paths: &[CellPath], row: Option<usize>) -> Result<(), ShellError> {
    if paths.is_empty() {
        return check(value, row, None);
    }
    for path in paths {
        // A path that is not there: Nushell's own says so.
        if let Ok(cell) = value.follow_cell_path(&path.members) {
            check(&cell, row, Some(path))?;
        }
    }
    Ok(())
}

fn check(value: &Value, row: Option<usize>, path: Option<&CellPath>) -> Result<(), ShellError> {
    let Value::String { val, .. } = value else {
        return Ok(());
    };
    let text = val.trim();
    let place = match (row, path) {
        (Some(row), Some(path)) => format!("row {row}, {}", path.to_column_name()),
        (Some(row), None) => format!("item {row}"),
        (None, Some(path)) => path.to_column_name(),
        (None, None) => "this text".into(),
    };
    let column = path.map(|path| path.to_column_name());
    if text.contains(',') {
        let fix = match &column {
            Some(column) => format!("update {column} {{ str replace --all ',' '' }}"),
            None => "str replace --all ',' ''".into(),
        };
        return Err(ShellError::Generic(
            GenericError::new(format!("{val:?} has a comma: thousands, or a decimal comma?"), place, value.span())
                .with_code("nux::into_float::comma")
                .with_help(format!(
                    "into float does not guess. Thousands: remove the commas first ({fix}); a decimal comma: \
                     make it a dot (str replace ',' '.')."
                )),
        ));
    }
    if text.parse::<f64>().is_err() {
        let help = match &column {
            Some(column) => format!(
                "What is not a number as null instead: update {column} {{|row| try {{ $row.{column} | into float }} catch {{ null }} }}"
            ),
            None => "What is not a number as null instead: try { $text | into float } catch { null }".into(),
        };
        return Err(ShellError::Generic(
            GenericError::new(format!("{val:?} is not a number"), place, value.span())
                .with_code("nu::shell::cant_convert")
                .with_help(help),
        ));
    }
    Ok(())
}
