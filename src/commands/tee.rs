//! `tee` on one value (a record, a number, a date) finishes its closure before the value goes
//! on. Nushell's runs that closure on a thread no one waits for ("we have nowhere to send an
//! error to"): nux could end before it did, so `{...} | tee { to json | save answer.json }`
//! left no file or an empty one, and an error in the closure was lost.
//!
//! Lists, text, binary and external output (what Nushell streams) go to Nushell's own `tee`,
//! which does wait.

use nu_engine::{ClosureEvalOnce, command_prelude::*};
use nu_protocol::{OutDest, engine::Closure};

#[derive(Clone)]
pub struct Tee;

impl Command for Tee {
    fn name(&self) -> &str {
        "tee"
    }
    fn description(&self) -> &str {
        nu_command::Tee.description()
    }
    fn extra_description(&self) -> &str {
        "In nux, on one value (a record, a number) the closure runs to its end before the value goes on, \
         and an error in it is an error of the call."
    }
    fn signature(&self) -> Signature {
        nu_command::Tee.signature()
    }
    fn search_terms(&self) -> Vec<&str> {
        nu_command::Tee.search_terms()
    }
    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            description: "Save a record and go on with it",
            example: "{a: 1} | tee { to json | save --force a.json } | get a",
            result: None,
        }]
    }
    fn pipe_redirection(&self) -> (Option<OutDest>, Option<OutDest>) {
        nu_command::Tee.pipe_redirection()
    }
    fn run(&self, engine_state: &EngineState, stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let one_value = matches!(&input, PipelineData::Value(value, _) if !matches!(value, Value::List { .. } | Value::String { .. } | Value::Binary { .. }));
        if !one_value || call.has_flag(engine_state, stack, "stderr")? {
            return nu_command::Tee.run(engine_state, stack, call, input);
        }
        let closure: Closure = call.req(engine_state, stack, 0)?;
        let mut input = input;
        let metadata = input.take_metadata();
        let value = input.into_value(call.head)?;
        ClosureEvalOnce::try_new(engine_state, stack, closure, call.head)?
            .run_with_input(PipelineData::value(value.clone(), metadata.clone()))?
            .drain()?;
        Ok(PipelineData::value(value, metadata))
    }
}
