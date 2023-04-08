use comemo::Track;
use typst::eval::{Route, Tracer, Value};
use typst::syntax::{ast, LinkedNode, Source, SyntaxKind};
use typst::World;

/// Try to determine a set of possible values for an expression.
///
/// From `typst::ide::analyze::analyze_expr`, but replacing the call to `World::main` with `source`
pub fn analyze_expr(
    world: &(dyn World + 'static),
    source: &Source,
    node: &LinkedNode,
) -> Vec<Value> {
    match node.cast::<ast::Expr>() {
        Some(ast::Expr::None(_)) => vec![Value::None],
        Some(ast::Expr::Auto(_)) => vec![Value::Auto],
        Some(ast::Expr::Bool(v)) => vec![Value::Bool(v.get())],
        Some(ast::Expr::Int(v)) => vec![Value::Int(v.get())],
        Some(ast::Expr::Float(v)) => vec![Value::Float(v.get())],
        Some(ast::Expr::Numeric(v)) => vec![Value::numeric(v.get())],
        Some(ast::Expr::Str(v)) => vec![Value::Str(v.get().into())],

        Some(ast::Expr::FieldAccess(access)) => {
            let Some(child) = node.children().next() else { return vec![] };
            analyze_expr(world, source, &child)
                .into_iter()
                .filter_map(|target| target.field(&access.field()).ok())
                .collect()
        }

        Some(_) => {
            if let Some(parent) = node.parent() {
                if parent.kind() == SyntaxKind::FieldAccess && node.index() > 0 {
                    return analyze_expr(world, source, parent);
                }
            }

            let route = Route::default();
            let mut tracer = Tracer::new(Some(node.span()));
            typst::eval::eval(world.track(), route.track(), tracer.track_mut(), source)
                .and_then(|module| {
                    typst::model::typeset(world.track(), tracer.track_mut(), &module.content())
                })
                .ok();

            tracer.finish()
        }

        _ => vec![],
    }
}
