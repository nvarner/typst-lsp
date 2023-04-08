//! Workarounds for calls to `World::main`. See the source of `world.rs` for more details.
//! The functions in the module are Typst's, with at most minor modifications to accept a `Source`
//! in place of a call to `World::main`. See https://github.com/typst/typst.

use comemo::Track;
use typst::diag::SourceResult;
use typst::doc::Document;
use typst::eval::{self, Route, Tracer};
use typst::syntax::Source;
use typst::{model, World};

pub mod ide;

/// From `typst::compile`, but replacing the call to `World::main` with `source`
pub fn compile(world: &(dyn World + 'static), source: &Source) -> SourceResult<Document> {
    // Evaluate the source file into a module.
    let route = Route::default();
    let mut tracer = Tracer::default();
    let module = eval::eval(world.track(), route.track(), tracer.track_mut(), source)?;

    // Typeset the module's contents.
    model::typeset(world.track(), tracer.track_mut(), &module.content())
}
