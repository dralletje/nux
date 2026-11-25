//! Text to numbers, without guessing about commas and dots.
//!
//! Nushell's own `into float` read a comma as a decimal comma ("1,500" became 1.5) and its
//! `into int` read it as a thousands separator ("1,5" became 15), without a word. Here:
//!
//! - `into float`, `into int`: as the US writes numbers. A comma groups thousands (1,500.25),
//!   and a comma anywhere else is an error.
//! - `into float-eu`, `into int-eu`: as much of Europe writes them. A dot groups thousands, a
//!   comma is the decimal one (1.500,25); anything else with a dot or comma is an error.
//!
//! Each error names the value, its row and column, and the other command. A value that is
//! not a number at all is named too (Nushell's says "can't convert string to float" near
//! where the table came from). The texts are checked and rewritten first (1,500.25 becomes
//! 1500.25); then Nushell's own converts them, as before.

use nu_engine::command_prelude::*;
use nu_protocol::{ast::CellPath, shell_error::generic::GenericError};

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Us,
    Eu,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Float,
    Int,
}

#[derive(Clone)]
pub struct IntoFloat;

#[derive(Clone)]
pub struct IntoInt;

#[derive(Clone)]
pub struct IntoFloatEu;

#[derive(Clone)]
pub struct IntoIntEu;

impl Command for IntoFloat {
    fn name(&self) -> &str {
        "into float"
    }
    fn description(&self) -> &str {
        nu_command::IntoFloat.description()
    }
    fn extra_description(&self) -> &str {
        "In nux a text is read as the US writes numbers: a comma groups thousands (\"1,500.25\" is \
         1500.25), and a comma anywhere else is an error (\"1,5\"). For 1.500,25 and 1,5: into float-eu. \
         A text that is not a number is named in the error, with its row."
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
            Example { description: "Commas between thousands", example: "'1,500.25' | into float", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let input = rewrite(engine_state, stack, call, input, Style::Us, Kind::Float)?;
        nu_command::IntoFloat.run(engine_state, stack, call, input)
    }
}

impl Command for IntoInt {
    fn name(&self) -> &str {
        "into int"
    }
    fn description(&self) -> &str {
        nu_command::IntoInt.description()
    }
    fn extra_description(&self) -> &str {
        "In nux a text is read as the US writes numbers: a comma groups thousands (\"1,500\" is 1500), \
         and a comma anywhere else is an error (\"1,5\"). For 1.500: into int-eu. --radix other than 10 \
         reads the text as Nushell does."
    }
    fn signature(&self) -> Signature {
        nu_command::IntoInt.signature()
    }
    fn search_terms(&self) -> Vec<&str> {
        nu_command::IntoInt.search_terms()
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "Convert a column of a table", example: "[[num]; ['-5']] | into int num", result: None },
            Example { description: "Commas between thousands", example: "'1,500' | into int", result: None },
            Example { description: "Another base", example: "'ff' | into int --radix 16", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        // Another base has its own digits: Nushell's own reads those.
        let radix: Option<i64> = call.get_flag(engine_state, stack, "radix")?;
        let input = if radix.is_none_or(|radix| radix == 10) {
            rewrite(engine_state, stack, call, input, Style::Us, Kind::Int)?
        } else {
            input
        };
        nu_command::IntoInt.run(engine_state, stack, call, input)
    }
}

impl Command for IntoFloatEu {
    fn name(&self) -> &str {
        "into float-eu"
    }
    fn description(&self) -> &str {
        "Convert text written the European way to a float: a dot groups thousands, a comma is the decimal one (1.500,25)."
    }
    fn extra_description(&self) -> &str {
        "\"1,5\" is 1.5 and \"1.500\" is 1500. A text with a dot or comma elsewhere is an error (\"1.5\"): for \
         numbers written the US way, into float."
    }
    fn signature(&self) -> Signature {
        renamed(nu_command::IntoFloat.signature(), self.name())
    }
    fn search_terms(&self) -> Vec<&str> {
        vec!["european", "decimal comma", "number", "locale"]
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "Convert a column of a table", example: "[[price]; ['12,50']] | into float-eu price", result: None },
            Example { description: "Dots between thousands", example: "'1.500,25' | into float-eu", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let input = rewrite(engine_state, stack, call, input, Style::Eu, Kind::Float)?;
        nu_command::IntoFloat.run(engine_state, stack, call, input)
    }
}

impl Command for IntoIntEu {
    fn name(&self) -> &str {
        "into int-eu"
    }
    fn description(&self) -> &str {
        "Convert text written the European way to an int: a dot groups thousands (1.500)."
    }
    fn extra_description(&self) -> &str {
        "\"1.500\" is 1500. A text with a comma, or a dot elsewhere, is an error: for numbers written the US \
         way, into int."
    }
    fn signature(&self) -> Signature {
        // Without --radix, --endian and --signed: those are for other bases and binary.
        let mut signature = renamed(nu_command::IntoInt.signature(), self.name());
        signature.named.retain(|flag| flag.long == "help");
        signature
    }
    fn search_terms(&self) -> Vec<&str> {
        vec!["european", "number", "locale"]
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example { description: "Dots between thousands", example: "'1.234.567' | into int-eu", result: None }]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let input = rewrite(engine_state, stack, call, input, Style::Eu, Kind::Int)?;
        nu_command::IntoInt.run(engine_state, stack, call, input)
    }
}

fn renamed(mut signature: Signature, name: &str) -> Signature {
    signature.name = name.into();
    signature
}

/// The input with its texts at the call's cell paths (or all of it, without paths) checked
/// and written as Nushell's own reads them: 1,500.25 (US) and 1.500,25 (EU) as 1500.25.
fn rewrite(engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData, style: Style, kind: Kind) -> Result<PipelineData, ShellError> {
    let paths: Vec<CellPath> = call.rest(engine_state, stack, 0)?;
    let mut input = input;
    let metadata = input.take_metadata();
    let mut value = input.into_value(call.head)?;
    match &mut value {
        Value::List { vals, .. } => {
            for (row, item) in vals.to_mut().iter_mut().enumerate() {
                rewrite_at(item, &paths, Some(row), style, kind)?;
            }
        }
        other => rewrite_at(other, &paths, None, style, kind)?,
    }
    Ok(PipelineData::value(value, metadata))
}

/// One row (or a value not in a list), at the given paths, or the value itself without paths.
fn rewrite_at(value: &mut Value, paths: &[CellPath], row: Option<usize>, style: Style, kind: Kind) -> Result<(), ShellError> {
    if paths.is_empty() {
        if let Some(new) = rewritten(value, row, None, style, kind)? {
            *value = new;
        }
        return Ok(());
    }
    for path in paths {
        // A path that is not there: Nushell's own says so.
        let Ok(cell) = value.follow_cell_path(&path.members) else {
            continue;
        };
        if let Some(new) = rewritten(&cell, row, Some(path), style, kind)? {
            value.update_cell_path(&path.members, Box::new(move |_| new))?;
        }
    }
    Ok(())
}

/// The text written as Nushell's own reads it, when that is different; an error when it is
/// not a number in this style.
fn rewritten(value: &Value, row: Option<usize>, path: Option<&CellPath>, style: Style, kind: Kind) -> Result<Option<Value>, ShellError> {
    let Value::String { val, .. } = value else {
        return Ok(None);
    };
    let column = path.map(|path| path.to_column_name());
    let place = match (row, &column) {
        (Some(row), Some(column)) => format!("row {row}, {column}"),
        (Some(row), None) => format!("item {row}"),
        (None, Some(column)) => column.clone(),
        (None, None) => "this text".into(),
    };
    let command = match (style, kind) {
        (Style::Us, Kind::Float) => "into float",
        (Style::Us, Kind::Int) => "into int",
        (Style::Eu, Kind::Float) => "into float-eu",
        (Style::Eu, Kind::Int) => "into int-eu",
    };
    let Some(text) = normalize(val.trim(), style, kind) else {
        let (what, example, why, other) = match (style, kind) {
            (Style::Us, Kind::Float) => ("a number", "1,500.25", "A comma groups thousands here.", "For a decimal comma (1,5 is one and a half): into float-eu."),
            (Style::Us, Kind::Int) => ("a whole number", "1,500", "A comma groups thousands here.", "For dots between thousands (1.500): into int-eu."),
            (Style::Eu, Kind::Float) => ("a number", "1.500,25", "A dot groups thousands here.", "For a decimal point (1.5 is one and a half): into float."),
            (Style::Eu, Kind::Int) => ("a whole number", "1.500", "A dot groups thousands here.", "For commas between thousands (1,500): into int."),
        };
        let style_name = if style == Style::Us { "US" } else { "European" };
        return Err(ShellError::Generic(
            GenericError::new(format!("{val:?} is not {what} in {style_name} style ({example})"), place, value.span())
                .with_code("nux::into::number_style")
                .with_help(format!("{why} {other}")),
        ));
    };
    if kind == Kind::Float && text.parse::<f64>().is_err() {
        let help = match &column {
            Some(column) => format!("What is not a number as null instead: update {column} {{|row| try {{ $row.{column} | {command} }} catch {{ null }} }}"),
            None => format!("What is not a number as null instead: try {{ $text | {command} }} catch {{ null }}"),
        };
        return Err(ShellError::Generic(
            GenericError::new(format!("{val:?} is not a number"), place, value.span()).with_code("nu::shell::cant_convert").with_help(help),
        ));
    }
    Ok((text != *val).then(|| Value::string(text, value.span())))
}

/// The text as Nushell's own reads it (no separators, a decimal point), or None when it has a
/// comma or dot that this style does not allow there. Text without either is left as it is.
fn normalize(text: &str, style: Style, kind: Kind) -> Option<String> {
    let (sign, rest) = match text.strip_prefix(['+', '-']) {
        Some(rest) => (&text[..1], rest),
        None => ("", text),
    };
    match (style, kind) {
        (Style::Us, _) if !text.contains(',') => Some(text.into()),
        (Style::Eu, _) if !text.contains([',', '.']) => Some(text.into()),
        (Style::Us, Kind::Int) => grouped(rest, ',').then(|| format!("{sign}{}", rest.replace(',', ""))),
        (Style::Us, Kind::Float) => {
            let (whole, fraction) = split_fraction(rest, '.');
            (grouped(whole, ',') && fraction.is_none_or(digits)).then(|| format!("{sign}{}{}", whole.replace(',', ""), point(fraction)))
        }
        (Style::Eu, Kind::Int) => grouped(rest, '.').then(|| format!("{sign}{}", rest.replace('.', ""))),
        (Style::Eu, Kind::Float) => {
            let (whole, fraction) = split_fraction(rest, ',');
            let whole_ok = if whole.contains('.') { grouped(whole, '.') } else { digits(whole) };
            (whole_ok && fraction.is_none_or(digits)).then(|| format!("{sign}{}{}", whole.replace('.', ""), point(fraction)))
        }
    }
}

fn split_fraction(text: &str, decimal: char) -> (&str, Option<&str>) {
    match text.split_once(decimal) {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    }
}

fn point(fraction: Option<&str>) -> String {
    fraction.map(|fraction| format!(".{fraction}")).unwrap_or_default()
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// Digits in groups of three after a first group of one to three: 1,500 or 12.345.678.
fn grouped(text: &str, separator: char) -> bool {
    let mut groups = text.split(separator);
    let first = groups.next().unwrap_or_default();
    let mut rest = groups.peekable();
    digits(first) && first.len() <= 3 && rest.peek().is_some() && rest.all(|group| group.len() == 3 && digits(group))
}
