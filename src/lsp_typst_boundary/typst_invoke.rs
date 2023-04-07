use comemo::Track;
use typst::diag::SourceResult;
use typst::doc::Document;
use typst::eval::{self, Route, Tracer};
use typst::syntax::Source;
use typst::{model, World};

/// From `typst::compile`, but replacing the call to `World::main` with `source`
pub fn compile(world: &(dyn World + 'static), source: &Source) -> SourceResult<Document> {
    // Evaluate the source file into a module.
    let route = Route::default();
    let mut tracer = Tracer::default();
    let module = eval::eval(world.track(), route.track(), tracer.track_mut(), source)?;

    // Typeset the module's contents.
    model::typeset(world.track(), tracer.track_mut(), &module.content())
}
