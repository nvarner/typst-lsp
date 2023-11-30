use std::sync::Arc;

use comemo::Track;
use tower_lsp::lsp_types::Url;
use typst::engine::Route;
use typst::eval::Tracer;
use typst::foundations::Module;
use typst::model::Document;
use typst::World;

use crate::lsp_typst_boundary::typst_to_lsp;

use super::diagnostics::DiagnosticsMap;
use super::TypstServer;

impl TypstServer {
    #[tracing::instrument(skip(self, uri), fields(%uri))]
    pub async fn compile_source(
        &self,
        uri: &Url,
    ) -> anyhow::Result<(Option<Arc<Document>>, DiagnosticsMap)> {
        let doc = self
            .scope_with_source(uri)
            .await?
            .run2(|source, project| async move {
                let (document, diagnostics) = self
                    .thread_with_world((source, project.clone()))
                    .await?
                    .run(|world| {
                        comemo::evict(30);

                        let mut tracer = Tracer::default();
                        let result = typst::compile(&world, &mut tracer);

                        let mut diagnostics = tracer.warnings();
                        match result {
                            Ok(document) => (Some(Arc::new(document)), diagnostics),
                            Err(errors) => {
                                diagnostics.extend_from_slice(&errors);
                                (None, diagnostics)
                            }
                        }
                    })
                    .await;

                let diagnostics =
                    typst_to_lsp::diagnostics(&project, diagnostics.as_ref(), self.const_config())
                        .await;

                let res: anyhow::Result<(Option<Arc<Document>>, DiagnosticsMap)> =
                    Ok((document, diagnostics));
                res
            })
            .await?;
        if let Some(doc) = &doc.0 {
            *self.document.lock().await = doc.clone();
        }
        Ok(doc)
    }

    #[tracing::instrument(skip(self, uri), fields(%uri))]
    pub async fn eval_source(&self, uri: &Url) -> anyhow::Result<(Option<Module>, DiagnosticsMap)> {
        let result = self
            .thread_with_world(uri)
            .await?
            .run(|world| {
                comemo::evict(30);

                let route = Route::default();
                let mut tracer = Tracer::default();
                typst::eval::eval(
                    (&world as &dyn World).track(),
                    route.track(),
                    tracer.track_mut(),
                    &world.main(),
                )
            })
            .await;

        let (module, errors) = match result {
            Ok(module) => (Some(module), Default::default()),
            Err(errors) => (Default::default(), errors),
        };

        let (project, _) = self.project_and_full_id(uri).await?;
        let diagnostics =
            typst_to_lsp::diagnostics(&project, errors.as_ref(), self.const_config()).await;

        Ok((module, diagnostics))
    }
}
