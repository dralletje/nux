//! Text to numbers, without guessing about commas and dots.
//!
//! Nushell's own `into float` read a comma as a decimal comma ("1,500" became 1.5) and its
//! `into int` read it as a thousands separator ("1,5" became 15), without a word. Here:
//!
//! - `into float`, `into int`: a decimal point. A comma groups thousands (1,500.25), and a
//!   comma anywhere else is an error.
//! - with `--decimal=comma`: as much of Europe writes numbers. A dot groups thousands, a comma
//!   is the decimal one (1.500,25); a dot or comma anywhere else is an error.
//!
//! Each error names the value, its row and column, and the other way. A value that is not a
//! number at all is named too (Nushell's says "can't convert string to float" near where the
//! table came from). The texts are checked and rewritten first (1,500.25 becomes 1500.25);
//! then Nushell's own converts them, as before.

use nu_engine::command_prelude::*;
use nu_protocol::{ast::CellPath, shell_error::generic::GenericError};

#[derive(Clone, Copy, PartialEq)]
enum Style {
    Point,
    Comma,
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

/// `--decimal=point` (the default) or `--decimal=comma`: which one a number''s decimal mark is.
/// One flag with two values, so a guess of the other one (`--decimal=point`) is right too.
const DECIMAL: &str = "decimal";

fn with_decimal(signature: Signature) -> Signature {
    signature.named(
        DECIMAL,
        SyntaxShape::String,
        "point (the default: 1,500.25) or comma (1.500,25): the decimal mark; the other one groups thousands.",
        None,
    )
}

impl Command for IntoFloat {
    fn name(&self) -> &str {
        "into float"
    }
    fn description(&self) -> &str {
        nu_command::IntoFloat.description()
    }
    fn extra_description(&self) -> &str {
        "In nux a comma in a text groups thousands (\"1,500.25\" is 1500.25), and a comma anywhere else \
         is an error (\"1,5\"). With --decimal=comma it is the other way around: \"1.500,25\" is 1500.25, \
         \"1,5\" is 1.5. A text that is not a number is named in the error, with its row."
    }
    fn signature(&self) -> Signature {
        with_decimal(nu_command::IntoFloat.signature())
    }
    fn search_terms(&self) -> Vec<&str> {
        let mut terms = nu_command::IntoFloat.search_terms();
        terms.extend(["decimal comma", "european", "thousands separator"]);
        terms
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "Convert a column of a table", example: "[[num]; ['5.01']] | into float num", result: None },
            Example { description: "Commas between thousands", example: "'1,500.25' | into float", result: None },
            Example { description: "A decimal comma, dots between thousands", example: "'1.500,25' | into float --decimal=comma", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let style = style(engine_state, stack, call)?;
        let input = rewrite(engine_state, stack, call, input, style, Kind::Float)?;
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
        "In nux a comma in a text groups thousands (\"1,500\" is 1500), and a comma anywhere else is an \
         error (\"1,5\"). With --decimal=comma a dot groups thousands: \"1.500\" is 1500. --radix other \
         than 10 reads the text as Nushell does."
    }
    fn signature(&self) -> Signature {
        with_decimal(nu_command::IntoInt.signature())
    }
    fn search_terms(&self) -> Vec<&str> {
        let mut terms = nu_command::IntoInt.search_terms();
        terms.extend(["european", "thousands separator"]);
        terms
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "Convert a column of a table", example: "[[num]; ['-5']] | into int num", result: None },
            Example { description: "Commas between thousands", example: "'1,500' | into int", result: None },
            Example { description: "Dots between thousands", example: "'1.500' | into int --decimal=comma", result: None },
            Example { description: "Another base", example: "'ff' | into int --radix 16", result: None },
        ]
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let style = style(engine_state, stack, call)?;
        // Another base has its own digits: Nushell's own reads those.
        let radix: Option<i64> = call.get_flag(engine_state, stack, "radix")?;
        let input = if radix.is_none_or(|radix| radix == 10) {
            rewrite(engine_state, stack, call, input, style, Kind::Int)?
        } else {
            input
        };
        nu_command::IntoInt.run(engine_state, stack, call, input)
    }
}

fn style(engine_state: &EngineState, stack: &mut Stack, call: &Call) -> Result<Style, ShellError> {
    let decimal: Option<Spanned<String>> = call.get_flag(engine_state, stack, DECIMAL)?;
    match decimal {
        None => Ok(Style::Point),
        Some(decimal) => match decimal.item.as_str() {
            "point" => Ok(Style::Point),
            "comma" => Ok(Style::Comma),
            other => Err(ShellError::Generic(
                GenericError::new(
                    format!("--decimal is point or comma, not {other:?}"),
                    "point or comma",
                    decimal.span,
                )
                .with_help("--decimal=point (the default) reads 1,500.25; --decimal=comma reads 1.500,25."),
            )),
        },
    }
}

/// The input with its texts at the call's cell paths (or all of it, without paths) checked
/// and written as Nushell's own reads them: 1,500.25 (and 1.500,25 with --decimal=comma) as
/// 1500.25.
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
/// not a number written this way.
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
        (Style::Point, Kind::Float) => "into float",
        (Style::Point, Kind::Int) => "into int",
        (Style::Comma, Kind::Float) => "into float --decimal=comma",
        (Style::Comma, Kind::Int) => "into int --decimal=comma",
    };
    let Some(text) = normalize(val.trim(), style, kind) else {
        let (what, help) = match (style, kind) {
            (Style::Point, Kind::Float) => (
                "a number with a decimal point (1,500.25)",
                "A comma groups thousands here. For a decimal comma (1,5 is one and a half): into float --decimal=comma.",
            ),
            (Style::Point, Kind::Int) => (
                "a whole number with commas between thousands (1,500)",
                "A comma groups thousands here. For dots between thousands (1.500): into int --decimal=comma.",
            ),
            (Style::Comma, Kind::Float) => (
                "a number with a decimal comma (1.500,25)",
                "A dot groups thousands here. For a decimal point (1.5 is one and a half): into float --decimal=point.",
            ),
            (Style::Comma, Kind::Int) => (
                "a whole number with dots between thousands (1.500)",
                "A dot groups thousands here. For commas between thousands (1,500): into int --decimal=point.",
            ),
        };
        return Err(ShellError::Generic(
            GenericError::new(format!("{val:?} is not {what}"), place, value.span())
                .with_code("nux::into::number_style")
                .with_help(help),
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
/// comma or dot where this way of writing does not allow one. Text without either is left as
/// it is.
fn normalize(text: &str, style: Style, kind: Kind) -> Option<String> {
    let (sign, rest) = match text.strip_prefix(['+', '-']) {
        Some(rest) => (&text[..1], rest),
        None => ("", text),
    };
    match (style, kind) {
        (Style::Point, _) if !text.contains(',') => Some(text.into()),
        (Style::Comma, _) if !text.contains([',', '.']) => Some(text.into()),
        (Style::Point, Kind::Int) => grouped(rest, ',').then(|| format!("{sign}{}", rest.replace(',', ""))),
        (Style::Point, Kind::Float) => {
            let (whole, fraction) = split_fraction(rest, '.');
            (grouped(whole, ',') && fraction.is_none_or(digits)).then(|| format!("{sign}{}{}", whole.replace(',', ""), point(fraction)))
        }
        (Style::Comma, Kind::Int) => grouped(rest, '.').then(|| format!("{sign}{}", rest.replace('.', ""))),
        (Style::Comma, Kind::Float) => {
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
