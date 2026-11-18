//! `to readable`: a value as nux prints it (src/readable.rs). For `show` in the prelude, so
//! that what it prints looks like a call's value; and for code that wants the same text.

use nu_engine::command_prelude::*;

#[derive(Clone)]
pub struct ToReadable;

impl Command for ToReadable {
    fn name(&self) -> &str {
        "to readable"
    }

    fn description(&self) -> &str {
        "A value as nux prints it: text as it is, a table as Markdown, a record as `key: value` lines, a list of texts one per line."
    }

    fn extra_description(&self) -> &str {
        "The form nux prints a call's values in. With NUX_FORMAT=nuon in the environment, NUON. The exact value, with its types: `to nuon`."
    }

    fn signature(&self) -> Signature {
        Signature::build("to readable").input_output_types(vec![(Type::Any, Type::String)]).category(Category::Formats)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["print", "format", "show", "text"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example { description: "A table, as nux would print it.", example: "ls | first 3 | to readable", result: None }]
    }

    fn run(&self, engine_state: &EngineState, _stack: &mut Stack, call: &Call, input: PipelineData) -> Result<PipelineData, ShellError> {
        let value = input.into_value(call.head)?;
        Ok(Value::string(crate::readable::shown(engine_state, &value), call.head).into_pipeline_data())
    }
}
