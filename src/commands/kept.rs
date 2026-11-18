//! Values kept for a next call: results too large to print, and the variables of a call that
//! failed. Every call is a fresh Nushell, so this is how something made in one call is used
//! in the next: `kept <name>`.
//!
//! Kept per agent conversation (Claude Code's CLAUDE_CODE_SESSION_ID, which subagents share),
//! one NUON file per value, so types survive. Nothing is removed: they are in the temp
//! directory, and a name used again replaces the value.

use std::path::PathBuf;

use nu_engine::command_prelude::*;
use nu_protocol::{did_you_mean, engine::EngineState, shell_error::generic::GenericError};

use crate::paths::nux_dir;

/// Where this conversation's values are.
pub fn dir() -> PathBuf {
    let conversation = std::env::var("CLAUDE_CODE_SESSION_ID")
        .ok()
        .filter(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or_else(|| "default".into());
    nux_dir().join("kept").join(conversation)
}

fn path_of(name: &str) -> PathBuf {
    dir().join(format!("{name}.nuon"))
}

/// A name nux may use as a file name: what a variable may be called, and `result-<n>`.
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn nuon(engine: &EngineState, value: &Value) -> Result<String, ShellError> {
    nuon::to_nuon(engine, value, nuon::ToNuonConfig::default().style(nuon::ToStyle::Raw).raw_strings(true))
}

/// Keep a value under a name. Fails for what NUON cannot hold (a closure, a polars frame).
pub fn keep(engine: &EngineState, name: &str, value: &Value) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("`{name}` cannot be a name"));
    }
    let text = nuon(engine, value).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(path_of(name), text).map_err(|e| e.to_string())
}

/// Keep a result too large to print as `result-<n>`, the next free number. Gives the name.
pub fn keep_result(engine: &EngineState, value: &Value) -> Option<String> {
    let text = nuon(engine, value).ok()?;
    std::fs::create_dir_all(dir()).ok()?;
    // Already kept (`kept result-3`, too large to print again): that name, not a new copy.
    if let Some(name) = already_kept(&text) {
        return Some(name);
    }
    // `create_new`: two calls at the same time cannot take the same number. A number that
    // is taken means: try the next one. Any other failure (a full disk) ends it, and leaves
    // no file behind.
    for n in 1..10_000 {
        let name = format!("result-{n}");
        let path = path_of(&name);
        let file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        };
        if std::io::Write::write_all(&mut &file, text.as_bytes()).is_err() {
            let _ = std::fs::remove_file(&path);
            return None;
        }
        return Some(name);
    }
    None
}

/// The kept result whose NUON is exactly this, if there is one. Files of another size are
/// not read.
fn already_kept(text: &str) -> Option<String> {
    std::fs::read_dir(dir()).ok()?.flatten().find_map(|entry| {
        let file_name = entry.file_name();
        let name = file_name.to_str()?.strip_suffix(".nuon")?;
        if !name.starts_with("result-") || entry.metadata().ok()?.len() != text.len() as u64 {
            return None;
        }
        (std::fs::read_to_string(entry.path()).ok()? == text).then(|| name.to_string())
    })
}

/// Keep the variables of a call that failed; says what was kept, for under the error.
pub fn keep_variables(engine: &EngineState, variables: &[(String, Value)]) -> String {
    let mut kept = Vec::new();
    let mut not_kept = Vec::new();
    for (name, value) in variables {
        match keep(engine, name, value) {
            Ok(()) => kept.push(format!("${name} ({})", describe(value))),
            Err(_) => not_kept.push(format!("${name} ({})", value.get_type())),
        }
    }
    let mut lines = Vec::new();
    if let Some((first, _)) = variables.iter().find(|(name, _)| kept.iter().any(|k| k.starts_with(&format!("${name} ")))) {
        lines.push(format!(
            "Kept from this call: {}. Get one back in a next call: let {first} = (kept {first})",
            kept.join(", ")
        ));
    }
    if !not_kept.is_empty() {
        lines.push(format!("Not kept (cannot be saved as data): {}", not_kept.join(", ")));
    }
    lines.join("\n")
}

/// A short description of a value: its type, and how many rows or fields.
pub fn describe(value: &Value) -> String {
    let mut kind = value.get_type().to_string();
    if kind.chars().count() > 120 {
        kind = format!("{}…", kind.chars().take(120).collect::<String>());
    }
    match value {
        // A table by its columns, not its full type: that gets long.
        Value::List { vals, .. } if kind.starts_with("table<") => {
            let columns: Vec<String> = vals.first().and_then(|row| row.as_record().ok()).map(|r| r.columns().cloned().collect()).unwrap_or_default();
            let shown = if columns.len() > 6 { format!("{}, …", columns[..6].join(", ")) } else { columns.join(", ") };
            format!("table, {} rows: {shown}", vals.len())
        }
        Value::List { vals, .. } => format!("{kind}, {} items", vals.len()),
        Value::Record { val, .. } => format!("{kind}, {} fields", val.len()),
        Value::String { val, .. } if val.lines().count() > 1 => format!("string, {} lines", val.lines().count()),
        _ => kind,
    }
}

#[derive(Clone)]
pub struct Kept;

impl Command for Kept {
    fn name(&self) -> &str {
        "kept"
    }

    fn description(&self) -> &str {
        "A value kept by an earlier nux call: a result too large to print, or a variable of a call that failed."
    }

    fn extra_description(&self) -> &str {
        "Every nux call is a fresh Nushell. What one call keeps, a next one gets back with `kept <name>`, \
         with its types (tables stay tables). Without a name: what is kept, as a table. Kept per agent \
         conversation. To keep a value yourself: `$rows | kept --save rows`."
    }

    fn signature(&self) -> Signature {
        Signature::build("kept")
            .input_output_types(vec![(Type::Nothing, Type::Any), (Type::Any, Type::Nothing)])
            .optional("name", SyntaxShape::String, "The name it was kept under, e.g. result-3 or a variable's name.")
            .named("save", SyntaxShape::String, "Keep the input under this name.", Some('s'))
            .switch("drop", "Remove the kept value of this name.", Some('d'))
            .category(Category::Custom("nux".into()))
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["saved", "result", "variable", "stash", "previous call"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example { description: "What is kept.", example: "kept", result: None },
            Example { description: "A large result, filtered.", example: "kept result-3 | where size > 1mb", result: None },
            Example { description: "A variable of a call that failed, back as a variable.", example: "let big = (kept big)", result: None },
            Example { description: "Keep a value yourself, for a next call.", example: "ls | kept --save files", result: None },
        ]
    }

    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let head = call.head;
        let name: Option<Spanned<String>> = call.opt(engine_state, stack, 0)?;
        let save: Option<Spanned<String>> = call.get_flag(engine_state, stack, "save")?;
        let drop = call.has_flag(engine_state, stack, "drop")?;

        if let Some(save) = save {
            let value = input.into_value(head)?;
            keep(engine_state, &save.item, &value)
                .map_err(|e| ShellError::Generic(GenericError::new(format!("Could not keep {}", save.item), e, save.span)))?;
            return Ok(PipelineData::empty());
        }
        let Some(name) = name else { return Ok(Value::list(list(), head).into_pipeline_data()) };
        let path = path_of(&name.item);
        if !valid_name(&name.item) || !path.exists() {
            let names: Vec<String> = list()
                .iter()
                .filter_map(|row| row.as_record().ok()?.get("name")?.as_str().ok().map(String::from))
                .collect();
            let help = match (did_you_mean(&names, &name.item), names.is_empty()) {
                (_, true) => "Nothing is kept in this conversation.".to_string(),
                (Some(close), _) => format!("Did you mean {close}? Kept: {}", names.join(", ")),
                (None, _) => format!("Kept: {}", names.join(", ")),
            };
            return Err(ShellError::Generic(
                GenericError::new(format!("Nothing is kept as `{}`", name.item), "this name", name.span).with_help(help),
            ));
        }
        if drop {
            std::fs::remove_file(&path).map_err(|e| ShellError::Generic(GenericError::new("Could not remove it", e.to_string(), name.span)))?;
            return Ok(PipelineData::empty());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| ShellError::Generic(GenericError::new("Could not read it", e.to_string(), name.span)))?;
        Ok(nuon::from_nuon(&text, Some(head))?.into_pipeline_data())
    }
}

/// What is kept: name, size, when; newest first.
fn list() -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(dir()) else { return Vec::new() };
    let mut rows: Vec<(std::time::SystemTime, Value)> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_str()?.strip_suffix(".nuon")?.to_string();
            let meta = entry.metadata().ok()?;
            let modified = meta.modified().ok()?;
            let when: chrono::DateTime<chrono::FixedOffset> = chrono::DateTime::<chrono::Local>::from(modified).fixed_offset();
            let row = record! {
                "name" => Value::string(name, Span::unknown()),
                "size" => Value::filesize(meta.len() as i64, Span::unknown()),
                "kept" => Value::date(when, Span::unknown()),
            };
            Some((modified, Value::record(row, Span::unknown())))
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.into_iter().map(|(_, row)| row).collect()
}
