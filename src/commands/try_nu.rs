//! `try-nu`: run Nushell code in a fresh nux and get what happened as data.
//!
//! What agents did with `^nu -c $code | complete` and then `from nuon` and scraping error
//! text: test a module after changing it (a fresh Nushell reads it again), see what a module
//! exports under `use` and `source`, try code that may fail. Here the code runs in a child
//! nux (same prelude, plugins, environment and directory); the value comes back with its
//! types, an error as a record.
//!
//! The child is this same binary, told by environment variables to write its outcome to a
//! file instead of printing it (see [`CHILD_RESULT`]). What it prints is the `printed` field.

use std::{
    ffi::OsString,
    io::Write,
    path::PathBuf,
    process::{Command as Process, Stdio},
};

use nu_engine::{command_prelude::*, env_to_strings};
use nu_protocol::shell_error::generic::GenericError;

use crate::paths::nux_dir;

/// Set for a child: where it writes `{value, error}` as NUON.
pub const CHILD_RESULT: &str = "NUX_TRY_RESULT";
/// Set for a child that has input: where the input is, as NUON.
pub const CHILD_INPUT: &str = "NUX_TRY_INPUT";
/// Set for a child: more directories for `use` and `source` (`NU_LIB_DIRS`), joined by `:`.
pub const CHILD_INCLUDE: &str = "NUX_TRY_INCLUDE";

/// What a child of `try-nu` was told.
pub struct ChildSettings {
    pub result: PathBuf,
    pub input: Option<PathBuf>,
    pub include: Option<OsString>,
}

/// The settings of a `try-nu` child, taken OUT of the environment: whatever this nux
/// starts (another nux, a nested `try-nu`) must not take them for its own. None when this
/// nux is not a child. Call it first thing in `main`: removing environment variables is
/// only sound while the process has a single thread.
pub fn take_child_settings() -> Option<ChildSettings> {
    let result = std::env::var_os(CHILD_RESULT);
    let input = std::env::var_os(CHILD_INPUT);
    let include = std::env::var_os(CHILD_INCLUDE);
    for name in [CHILD_RESULT, CHILD_INPUT, CHILD_INCLUDE] {
        // SAFETY: called before any other thread exists (see above).
        unsafe { std::env::remove_var(name) };
    }
    Some(ChildSettings { result: result?.into(), input: input.map(PathBuf::from), include })
}

#[derive(Clone)]
pub struct TryNu;

impl Command for TryNu {
    fn name(&self) -> &str {
        "try-nu"
    }

    fn description(&self) -> &str {
        "Run Nushell code in a fresh nux and get the outcome as a record: value, error, printed, stderr."
    }

    fn extra_description(&self) -> &str {
        "Instead of `^nu -c $code`. The code runs in a new Nushell with nux's prelude, in this directory \
         and environment, so a module you just changed is read again, and nothing it defines reaches this \
         call. The pipeline input is the code's `$in`. `value` keeps its types; `error` is null or a record \
         with msg, code, help, line, column (in the code given) and text (as nux would print it); `printed` \
         is what the code printed. A failing external command at the end is a value (`exit_code`), not an error."
    }

    fn signature(&self) -> Signature {
        Signature::build("try-nu")
            .input_output_types(vec![(Type::Any, Type::record())])
            .required("code", SyntaxShape::String, "The Nushell code to run.")
            .named(
                "include",
                SyntaxShape::List(Box::new(SyntaxShape::Filepath)),
                "Directories to find modules in (`use`, `source`), like `nu -I`.",
                Some('I'),
            )
            .category(Category::Custom("nux".into()))
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["nu -c", "eval", "sandbox", "fresh", "subshell", "test module", "catch"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                description: "Test a module after changing it.",
                example: "try-nu -I [packages/nux-cli/prelude] 'use git.nu; git diff | length'",
                result: None,
            },
            Example { description: "An error as data.", example: "try-nu 'open nope.json' | get error.msg", result: None },
            Example { description: "Input is the code's $in.", example: "[1 2 3] | try-nu '$in | math sum' | get value", result: None },
            Example {
                description: "What a module exports under each way of loading it.",
                example: "['use m.nu' 'use m.nu *' 'source m.nu'] | each {|how| {how: $how, names: (try-nu $\"($how); scope commands | where name =~ '^m' | get name\" | get value)} }",
                result: None,
            },
        ]
    }

    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let head = call.head;
        let code: Spanned<String> = call.req(engine_state, stack, 0)?;
        let include: Option<Vec<String>> = call.get_flag(engine_state, stack, "include")?;
        let failed = |what: &str, e: String| ShellError::Generic(GenericError::new(format!("try-nu: {what}"), e, head));

        let cwd = engine_state.cwd(Some(stack))?;
        let env = env_to_strings(engine_state, stack)?;
        let dir = nux_dir().join("try");
        std::fs::create_dir_all(&dir).map_err(|e| failed("no place for its files", e.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let result_file = dir.join(format!("{id}-result.nuon"));
        let input_file = dir.join(format!("{id}-input.nuon"));

        let mut child = Process::new(std::env::current_exe().map_err(|e| failed("cannot find nux itself", e.to_string()))?);
        child.env_clear().envs(&env).current_dir(cwd.as_std_path()).env(CHILD_RESULT, &result_file);
        let input = input.into_value(head)?;
        if !matches!(input, Value::Nothing { .. }) {
            let text = nuon::to_nuon(engine_state, &input, nuon::ToNuonConfig::default().style(nuon::ToStyle::Raw))?;
            std::fs::write(&input_file, text).map_err(|e| failed("could not pass the input", e.to_string()))?;
            child.env(CHILD_INPUT, &input_file);
        }
        if let Some(include) = include {
            let dirs: Vec<String> = include
                .iter()
                .map(|d| nu_path::absolute_with(d, cwd.as_std_path()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| d.clone()))
                .collect();
            child.env(CHILD_INCLUDE, dirs.join(":"));
        }
        let mut process = child
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| failed("could not start", e.to_string()))?;
        let _ = process.stdin.take().map(|mut stdin| stdin.write_all(code.item.as_bytes()));
        let output = process.wait_with_output().map_err(|e| failed("did not finish", e.to_string()))?;

        let text = |bytes: &[u8]| {
            let text = String::from_utf8_lossy(bytes);
            Value::string(text.strip_suffix('\n').unwrap_or(&text), head)
        };
        let outcome = std::fs::read_to_string(&result_file).ok().and_then(|t| nuon::from_nuon(&t, Some(head)).ok());
        let _ = std::fs::remove_file(&result_file);
        let _ = std::fs::remove_file(&input_file);
        let (value, error) = match outcome {
            Some(Value::Record { val, .. }) => (
                val.get("value").cloned().unwrap_or(Value::nothing(head)),
                val.get("error").cloned().unwrap_or(Value::nothing(head)),
            ),
            // No outcome: the child failed before it could write one (or was killed).
            _ => (
                Value::nothing(head),
                Value::record(
                    record! {
                        "msg" => Value::string(format!("the child nux ended without a result (exit status {:?})", output.status.code()), head),
                        "text" => text(&output.stderr),
                    },
                    head,
                ),
            ),
        };
        Ok(Value::record(
            record! {
                "value" => value,
                "error" => error,
                "printed" => text(&output.stdout),
                "stderr" => text(&output.stderr),
            },
            head,
        )
        .into_pipeline_data())
    }
}
