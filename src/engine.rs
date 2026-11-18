//! Nushell, set up for one call: the engine, the prelude, and running the code.

use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

use nu_engine::{convert_env_values, eval_block};
use nu_protocol::{
    Config, IntoValue, PipelineData, PipelineExecutionData, ShellError, Signals, Span, Type, UseAnsiColoring, Value, VarId,
    ast::Block,
    debugger::WithoutDebug,
    engine::{EngineState, Stack, StateDelta, StateWorkingSet},
    process::check_exit_status_future,
    record,
};

use crate::{
    assist::{self, Raw},
    commands::{self, kept},
    format::{self, CODE_FILE, Sources, format_completed, without_final_newline},
    prelude, readable,
    rewrite::{line_of, nu_raw_string, shorten, statements, with_exit_checks},
    usage_log::Outcome,
};

/// Results longer than this (as printed) are kept instead of printed. The same limit
/// Nushell's MCP server uses.
const OUTPUT_LIMIT: usize = 10 * 1024;

/// What a call comes to: what to print, and how it ended.
pub struct Reply {
    pub ok: bool,
    pub text: String,
    pub outcome: Outcome,
    pub error_code: Option<String>,
    /// The process's exit status: 0, 1, or what `exit` asked for.
    pub exit_code: i32,
}

impl Reply {
    pub fn ok(text: impl Into<String>) -> Self {
        Reply { ok: true, text: text.into(), outcome: Outcome::Ok, error_code: None, exit_code: 0 }
    }
    pub fn failed(outcome: Outcome, text: impl Into<String>, error_code: Option<String>) -> Self {
        Reply { ok: false, text: text.into(), outcome, error_code, exit_code: 1 }
    }
}

pub struct Nu {
    engine: EngineState,
    stack: Stack,
    /// Set when the prelude did not load: nux's own commands are missing then.
    pub prelude_problem: Option<String>,
    /// The code's last top-level statement: its line and how it is shown. Its value gets a
    /// line saying so when earlier values were shown (`statement_value`).
    last_statement: Option<(usize, String)>,
}

/// An error, already put into words (a parse error refers to code that is not in the engine
/// yet, so it has to be described while its working set is still there), and as data.
struct Failure {
    text: String,
    code: Option<String>,
    record: Value,
}

/// How code that ran came out.
enum Evaluated {
    Value(Value),
    /// The last statement was an external command: what it wrote, and how it exited.
    /// `command`: the command as shown in messages, when nux completed it.
    Completed { stdout: String, stderr: String, exit_code: i64, command: Option<String> },
}

/// How code that did not come to a value stopped.
enum Stopped {
    Failed(Failure),
    /// `exit <code>`
    Exit(i32),
}

impl Nu {
    /// A fresh Nushell, as `nu -n` would be, without colors, with the prelude loaded.
    /// `include`: more module directories, from `try-nu -I` (joined like PATH).
    pub fn start(cwd: &Path, interrupt: Arc<AtomicBool>, include: Option<std::ffi::OsString>) -> Result<Nu, String> {
        // TLS for `http` (https://): rustls needs a crypto provider chosen at startup, as `nu`
        // does in its main. Without it every https request fails.
        nu_command::tls::CRYPTO_PROVIDER.default();
        let mut engine = nu_cmd_lang::create_default_context();
        engine = nu_command::add_shell_command_context(engine);
        engine = nu_cmd_extra::add_extra_command_context(engine);
        engine = nu_cli::add_cli_context(engine);
        // `plugin add`, `plugin list`, ...: plugins can be managed without `nu`.
        engine = nu_cmd_plugin::add_plugin_command_context(engine);
        engine = commands::add(engine);

        // Nushell's own config paths, found as `nu` finds them. nux reads none of the config
        // files, but shares `nu`'s plugin registry (`$nu.plugin-path`).
        if let Ok((config_dirs, _warnings)) = nu_config::resolve_paths(&nu_config::SystemEnv, &Default::default()) {
            engine.config_dirs = config_dirs;
        }

        let mut config = Config::default();
        config.use_ansi_coloring = UseAnsiColoring::False;
        engine.add_env_var("config".into(), config.clone().into_value(Span::unknown()));
        engine.set_config(config);
        engine.add_env_var("ENV_CONVERSIONS".into(), Value::record(record! {}, Span::unknown()));
        nu_cli::gather_parent_env_vars(&mut engine, cwd);

        let mut stack = Stack::new();
        convert_env_values(&mut engine, &mut stack).map_err(|e| format!("Could not read the environment: {e}"))?;
        engine.add_env_var("NU_VERSION".into(), Value::string(NU_VERSION, Span::unknown()));
        if let Some(include) = include {
            set_lib_dirs(&mut engine, &include);
        }
        nu_std::load_standard_library(&mut engine).map_err(|e| format!("Could not load Nushell's standard library: {e}"))?;
        // For `stor`; failing only makes those commands fail.
        let _ = nu_command::init_shared_memory_db();
        engine.generate_nu_constant();
        // The registered plugins: only their signatures are read here; a plugin's process
        // starts when a call first uses one of its commands.
        nu_cli::read_plugin_file(&mut engine, None);
        engine.set_signals(Signals::new(interrupt));
        engine.finish_startup();

        // External commands at the end of a pipeline hand their output to nux, instead of
        // writing to the terminal.
        let mut nu = Nu { engine, stack: stack.capture_all(), prelude_problem: None, last_statement: None };
        match prelude::install(&mut nu.engine) {
            Ok(source) => {
                if let Err(failure) = nu.run_source("nux-prelude", &source) {
                    nu.prelude_problem = Some(failure.text);
                }
            }
            Err(problem) => nu.prelude_problem = Some(problem),
        }
        Ok(nu)
    }

    /// Parse code without running it. Gives the block, what parsing added to the engine, and
    /// where the code starts in the engine's span coordinates.
    fn parse(&self, name: &str, source: &str) -> Result<(Arc<Block>, StateDelta, usize), Failure> {
        let mut working_set = StateWorkingSet::new(&self.engine);
        let base = working_set.next_span_start();
        let block = nu_parser::parse(&mut working_set, Some(name), source.as_bytes(), false);
        if let Some(error) = working_set.parse_errors.first() {
            return Err(self.failure(Raw::Parse(error), &working_set));
        }
        if let Some(error) = working_set.compile_errors.first() {
            return Err(self.failure(Raw::Compile(error), &working_set));
        }
        Ok((block, working_set.render(), base))
    }

    fn shell_failure(&self, error: &ShellError) -> Failure {
        self.failure(Raw::Shell(error), &self.engine)
    }

    /// An error in words: Nushell's message, then nux's hints for it (see `assist`).
    fn failure(&self, raw: Raw, sources: &dyn Sources) -> Failure {
        let error: &dyn miette::Diagnostic = match &raw {
            Raw::Parse(e) => *e,
            Raw::Compile(e) => *e,
            Raw::Shell(e) => *e,
        };
        let hints = assist::error_hints(&raw, sources);
        let text = format::join(&[&format::format_error(error, sources), &hints]);
        let record = format::error_record(error, sources, &text);
        Failure { code: format::error_code(error), record, text }
    }

    /// The block's result, with the exit statuses of the external commands in its last pipeline
    /// (`exit`): check those only after the result has been read (`evaluated`), as `nu` does,
    /// or a command that fills its pipe never exits.
    fn run_block(&mut self, block: &Block, delta: StateDelta, input: PipelineData) -> Result<PipelineExecutionData, ShellError> {
        self.engine.merge_delta(delta)?;
        eval_block::<WithoutDebug>(&self.engine, &mut self.stack, block, input)
    }

    fn run_source(&mut self, name: &str, source: &str) -> Result<PipelineData, Failure> {
        let (block, delta, _) = self.parse(name, source)?;
        self.run_block(&block, delta, PipelineData::empty()).map(|data| data.body).map_err(|e| self.shell_failure(&e))
    }

    /// Run an agent's code the way nux promises (see `nux help`): external commands at the top
    /// level are completed, so that their output and exit code are seen.
    fn evaluate(&mut self, code: &str, input: PipelineData) -> Result<Evaluated, Stopped> {
        let (block, delta, base) = self.parse(CODE_FILE, code).map_err(Stopped::Failed)?;
        // Keywords are Nushell's own, so in the engine already; a command the code defines
        // itself is not (its id is past the engine's), and is no keyword.
        let engine = &self.engine;
        let keyword = |id: nu_protocol::DeclId| {
            (id.get() < engine.num_decls())
                .then(|| engine.get_decl(id))
                .filter(|decl| decl.is_keyword())
                .map(|decl| decl.name().to_string())
        };
        let found = statements(&block, base, keyword);
        self.last_statement = found.last().map(|s| (line_of(code, s.start), shorten(&code[s.start..s.end])));
        let checks = self.prelude_problem.is_none();
        // With the values of earlier statements shown; should that ever break the code, with
        // the exit checks only; should those break it too, the code as it was given.
        let mut rewritten = None;
        for show_values in [true, false] {
            let checked = with_exit_checks(code, &found, checks, show_values);
            if !checked.changed {
                break;
            }
            if let Ok((block, delta, _)) = self.parse(CODE_FILE, &checked.code) {
                rewritten = Some((block, delta, checked.last_command, checked.insertions));
                break;
            }
        }
        let (block, delta, last_command, insertions) = rewritten.unwrap_or((block, delta, None, Vec::new()));
        // Errors point at the code as the agent wrote it, not as nux ran it.
        let failed = |nu: &Self, error: &ShellError| {
            Stopped::Failed(nu.failure(Raw::Shell(error), &format::AsGiven { engine: &nu.engine, code, insertions: &insertions }))
        };
        let data = match self.run_block(&block, delta, input) {
            Ok(data) => data,
            Err(ShellError::Exit { code, .. }) => return Err(Stopped::Exit(code)),
            Err(error) => return Err(failed(self, &error)),
        };
        let evaluated = self.evaluated(data.body, last_command).map_err(|e| failed(self, &e))?;
        // An external command earlier in the last pipeline (`b | lines`, with `b` a def that
        // runs one) that failed: its failure, after the value it led to.
        match check_exit_status_future(data.exit) {
            Ok(()) => Ok(evaluated),
            Err(error) => {
                let Stopped::Failed(mut failure) = failed(self, &error) else { unreachable!() };
                if let Evaluated::Value(value) = &evaluated
                    && !matches!(value, Value::Nothing { .. })
                {
                    let shown = self.to_nuon(value);
                    failure.text = format!("What the pipeline gave before that:\n{shown}\n{}", failure.text);
                }
                Err(Stopped::Failed(failure))
            }
        }
    }

    fn evaluated(&self, body: PipelineData, last_command: Option<String>) -> Result<Evaluated, ShellError> {
        let value = match body {
            // An external command whose output nux did not complete (redirected, or in
            // code nux could not rewrite): its output, and its exit code when it has one.
            PipelineData::ByteStream(stream, ..) => match stream.into_child() {
                Ok(mut child) => {
                    child.ignore_error(true);
                    let output = child.wait_with_output()?;
                    return Ok(Evaluated::Completed {
                        stdout: String::from_utf8_lossy(&output.stdout.unwrap_or_default()).into_owned(),
                        stderr: String::from_utf8_lossy(&output.stderr.unwrap_or_default()).into_owned(),
                        exit_code: output.exit_status.code() as i64,
                        command: None,
                    });
                }
                Err(stream) => {
                    let mut buffer = Vec::new();
                    stream.write_to(&mut buffer)?;
                    Value::string(String::from_utf8_lossy(&buffer), Span::unknown())
                }
            },
            other => other.into_value(Span::unknown())?,
        };
        if let Value::Error { error, .. } = value {
            return Err(*error);
        }
        if let (Some(command), Value::Record { val, .. }) = (&last_command, &value) {
            if let (Some(stdout), Some(Value::Int { val: exit_code, .. })) = (val.get("stdout"), val.get("exit_code")) {
                let as_text = |v: Option<&Value>| match v {
                    Some(Value::String { val, .. }) => val.clone(),
                    Some(Value::Binary { val, .. }) => String::from_utf8_lossy(val).into_owned(),
                    _ => String::new(),
                };
                return Ok(Evaluated::Completed {
                    stdout: as_text(Some(stdout)),
                    stderr: as_text(val.get("stderr")),
                    exit_code: *exit_code,
                    command: Some(command.clone()),
                });
            }
        }
        Ok(Evaluated::Value(value))
    }

    /// Run the code an agent gave, and put the outcome into words.
    pub fn run_code(&mut self, code: &str) -> Reply {
        let before = self.variable_ids();
        let mut reply = match self.evaluate(code, PipelineData::empty()) {
            Ok(Evaluated::Value(value)) => self.value_reply(value),
            Ok(Evaluated::Completed { stdout, stderr, exit_code, command }) => {
                self.completed_reply(&stdout, &stderr, exit_code, command.as_deref())
            }
            Err(Stopped::Exit(code)) => {
                let mut reply = if code == 0 { Reply::ok("") } else { Reply::failed(Outcome::Error, "", None) };
                reply.exit_code = code;
                return reply;
            }
            Err(Stopped::Failed(failure)) => Reply::failed(Outcome::Error, failure.text, failure.code),
        };
        // Earlier values were shown, each under a line naming its statement: the last value too.
        if reply.ok
            && !reply.text.is_empty()
            && commands::statement_value::shown() > 0
            && let Some((line, statement)) = &self.last_statement
        {
            reply.text = format!("{}\n{}", commands::statement_value::header(*line, statement), reply.text);
        }
        // A call that failed loses its variables with it: keep them for a next call.
        if !reply.ok {
            let variables = self.new_variables(&before);
            if !variables.is_empty() {
                reply.text = format::join(&[&reply.text, &kept::keep_variables(&self.engine, &variables)]);
            }
        }
        reply
    }

    /// For `try-nu`: run the code as a child of another nux, and give `{value, error}`.
    pub fn run_for_result(&mut self, code: &str, input: Value) -> Value {
        let nothing = || Value::nothing(Span::unknown());
        let (value, error) = match self.evaluate(code, PipelineData::value(input, None)) {
            Ok(Evaluated::Value(value)) => (value, nothing()),
            Ok(Evaluated::Completed { stdout, stderr, exit_code, .. }) => (
                Value::record(
                    record! {
                        "stdout" => Value::string(stdout, Span::unknown()),
                        "stderr" => Value::string(stderr, Span::unknown()),
                        "exit_code" => Value::int(exit_code, Span::unknown()),
                    },
                    Span::unknown(),
                ),
                nothing(),
            ),
            Err(Stopped::Exit(0)) => (nothing(), nothing()),
            Err(Stopped::Exit(code)) => (
                nothing(),
                Value::record(record! { "msg" => Value::string(format!("the code ran `exit {code}`"), Span::unknown()) }, Span::unknown()),
            ),
            Err(Stopped::Failed(failure)) => (nothing(), failure.record),
        };
        Value::record(record! { "value" => value, "error" => error }, Span::unknown())
    }

    /// A value as NUON (`null` for what NUON cannot hold).
    pub fn to_nuon(&self, value: &Value) -> String {
        let config = nuon::ToNuonConfig::default().style(nuon::ToStyle::Raw);
        nuon::to_nuon(&self.engine, value, config).unwrap_or_else(|e| {
            // Give the error instead of a value NUON cannot hold, e.g. a closure.
            format!("{{value: null, error: {{msg: {}}}}}", serde_json::to_string(&format!("the value cannot be passed back: {e}")).unwrap_or_default())
        })
    }

    /// Variables known to the engine (in scope at the top level).
    fn variable_ids(&self) -> HashSet<VarId> {
        self.engine.active_overlays(&[]).flat_map(|overlay| overlay.vars.values().copied()).collect()
    }

    /// The top-level variables the code defined and set, in the order it defined them.
    fn new_variables(&self, before: &HashSet<VarId>) -> Vec<(String, Value)> {
        let mut found: Vec<(VarId, String, Value)> = self
            .engine
            .active_overlays(&[])
            .flat_map(|overlay| overlay.vars.iter())
            .filter(|(_, id)| !before.contains(id))
            .filter_map(|(name, id)| {
                let name = String::from_utf8_lossy(name).trim_start_matches('$').to_string();
                // `$in` is the pipeline input, which Nushell also keeps as a variable: not
                // one the code made (and `let in = ...` is not allowed).
                if name == "in" {
                    return None;
                }
                let value = self.stack.get_var(*id, Span::unknown()).ok()?;
                Some((*id, name, value))
            })
            .collect();
        found.sort_by_key(|(id, _, _)| id.get());
        found.into_iter().map(|(_, name, value)| (name, value)).collect()
    }

    fn value_reply(&self, value: Value) -> Reply {
        Reply::ok(value_text(&self.engine, &value))
    }

    fn completed_reply(&self, stdout: &str, stderr: &str, exit_code: i64, command: Option<&str>) -> Reply {
        let mut shown = stdout.to_string();
        if shown.len() > OUTPUT_LIMIT {
            if let Some(name) = kept::keep_result(&self.engine, &Value::string(stdout, Span::unknown())) {
                shown = assist::large_text(stdout, &name);
            }
        }
        let (ok, text) = format_completed(&shown, stderr, exit_code, command);
        if ok { Reply::ok(text) } else { Reply::failed(Outcome::ExternalFailed, text, None) }
    }

    /// `nux help <command>`: Nushell's help for one command.
    pub fn help(&mut self, words: &[String]) -> Reply {
        let source = format!("help {}", words.iter().map(|w| nu_raw_string(w)).collect::<Vec<_>>().join(" "));
        let mut result = self.run_source("nux-help", &source).and_then(|data| {
            data.into_value(Span::unknown()).map_err(|e| self.shell_failure(&e))
        });
        // A command of a module that is loaded only with `use` (`cookie-jar http`): with it.
        if result.is_err()
            && let Some(module) = words.first().filter(|w| w.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
            && let Ok(value) = self
                .run_source("nux-help", &format!("use nux/{module}\n{source}"))
                .and_then(|data| data.into_value(Span::unknown()).map_err(|e| self.shell_failure(&e)))
        {
            result = Ok(value);
        }
        match result {
            Ok(Value::String { val, .. }) => Reply::ok(val.trim().to_string()),
            Ok(other) => Reply::ok(other.to_expanded_string("\n", &self.engine.get_config())),
            Err(failure) => Reply::failed(
                Outcome::Error,
                format!(
                    "{}\nSearch for it with: nux find {}",
                    failure.text,
                    words.last().map(String::as_str).unwrap_or("<word>")
                ),
                failure.code,
            ),
        }
    }

    /// `nux find <word>`: commands whose name, description or search terms mention it.
    pub fn find(&mut self, term: &str) -> Reply {
        let source = format!(
            r#"let term = ({term} | str downcase)
help commands
| where {{|c| ($c.name | str downcase | str contains $term) or ($c.description | str downcase | str contains $term) or ($c.search_terms | str downcase | str contains $term) }}
| each {{|c| $"($c.name)  ($c.description | lines | get 0? | default '')" }}
| str join "\n""#,
            term = nu_raw_string(term)
        );
        match self.run_source("nux-find", &source).and_then(|d| d.into_value(Span::unknown()).map_err(|e| self.shell_failure(&e))) {
            Ok(Value::String { val, .. }) if val.is_empty() => Reply::ok(format!("No command mentions \"{term}\".")),
            Ok(Value::String { val, .. }) => Reply::ok(val),
            Ok(other) => Reply::ok(other.to_expanded_string("\n", &self.engine.get_config())),
            Err(failure) => Reply::failed(Outcome::Error, failure.text, failure.code),
        }
    }
}

/// A value as nux prints it: nothing for no value, text as it is, anything else as NUON. One
/// too large to print is kept (`kept result-<n>`), and what it looks like is shown instead.
pub fn value_text(engine: &EngineState, value: &Value) -> String {
    let text = match value {
        // "No value" (a `let`, a `cd`, a `print`) prints nothing.
        Value::Nothing { .. } => return String::new(),
        // Text (also what an external command printed, or `to json` made) is shown as it is.
        Value::String { val, .. } => without_final_newline(val).to_string(),
        // NUON, as before, when asked for (NUX_FORMAT=nuon); else made for reading.
        _ if readable::nuon_wanted() => readable::nuon(engine, value),
        _ => readable::readable(engine, value),
    };
    if text.len() <= OUTPUT_LIMIT {
        return text;
    }
    match kept::keep_result(engine, value) {
        Some(name) => match value {
            Value::String { .. } => assist::large_text(&text, &name),
            _ => assist::large_value(engine, value, &name, text.len()),
        },
        None => text,
    }
}

const NU_VERSION: &str = "0.117.0";

/// `NU_LIB_DIRS`, where `use` and `source` look for modules: the directories `try-nu -I`
/// passed, as the environment variable and as the constant `$NU_LIB_DIRS` (like `nu -I`).
fn set_lib_dirs(engine: &mut EngineState, include: &std::ffi::OsStr) {
    let dirs: Vec<Value> = std::env::split_paths(include)
        .map(|dir| Value::string(dir.to_string_lossy(), Span::unknown()))
        .collect();
    engine.add_env_var("NU_LIB_DIRS".into(), Value::list(dirs.clone(), Span::unknown()));
    let delta = {
        let mut working_set = StateWorkingSet::new(engine);
        let var_id = working_set.add_variable(b"$NU_LIB_DIRS".into(), Span::unknown(), Type::List(Box::new(Type::String)), false);
        working_set.set_variable_const_val(var_id, Value::list(dirs, Span::unknown()));
        working_set.render()
    };
    let _ = engine.merge_delta(delta);
}
