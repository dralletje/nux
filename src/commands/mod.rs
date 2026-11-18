//! nux's own commands written in Rust (the prelude has those written in Nushell).
//!
//! To add one: a file with a struct that implements Nushell's `Command`, and a line in
//! [`add`]. Everything else in nux stays as it is.

pub mod kept;
pub mod statement_value;
pub mod to_readable;
pub mod try_nu;

use nu_protocol::engine::{EngineState, StateWorkingSet};

/// Add nux's commands to an engine.
pub fn add(mut engine: EngineState) -> EngineState {
    let delta = {
        let mut working_set = StateWorkingSet::new(&engine);
        working_set.add_decl(Box::new(kept::Kept));
        working_set.add_decl(Box::new(try_nu::TryNu));
        working_set.add_decl(Box::new(statement_value::StatementValue));
        working_set.add_decl(Box::new(to_readable::ToReadable));
        working_set.render()
    };
    if let Err(error) = engine.merge_delta(delta) {
        eprintln!("nux: could not add its own commands: {error}");
    }
    engine
}
