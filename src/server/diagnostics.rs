use std::collections::HashMap;

use futures::future::join_all;
use tower_lsp::lsp_types::{Diagnostic, Url};
use tower_lsp::Client;

use super::TypstServer;

pub type DiagnosticsMap = HashMap<Url, Vec<Diagnostic>>;

impl TypstServer {
    pub async fn update_all_diagnostics(&self, diagnostics: DiagnosticsMap) {
        self.diagnostics.lock().await.publish(diagnostics).await;
    }
}

pub struct DiagnosticsManager {
    client: Client,
    last_published_for: Vec<Url>,
}

impl DiagnosticsManager {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            last_published_for: Vec::new(),
        }
    }

    pub async fn publish(&mut self, next_diagnostics: DiagnosticsMap) {
        let should_clear = self.should_clear(&next_diagnostics);
        self.push(should_clear).await;

        // We just used the cache, and won't need it again, so we can update it now
        self.update_cache(&next_diagnostics);

        self.push(next_diagnostics).await;
    }

    /// Gets sources which had some diagnostic published last time, but not this time. The LSP
    /// specifies that files will not have diagnostics updated, including removed, without an
    /// explicit update, so we need to send an empty `Vec` of diagnostics to these sources.
    fn should_clear<'a>(
        &'a self,
        next_diagnostics: &'a DiagnosticsMap,
    ) -> impl Iterator<Item = (Url, Vec<Diagnostic>)> + 'a {
        self.last_published_for
            .iter()
            .filter(|uri| !next_diagnostics.contains_key(uri))
            .cloned()
            .map(|uri| (uri, vec![]))
    }

    fn update_cache(&mut self, next_diagnostics: &DiagnosticsMap) {
        self.last_published_for.clear();
        self.last_published_for
            .extend(next_diagnostics.keys().cloned());
    }

    async fn push(&self, diagnostics: impl IntoIterator<Item = (Url, Vec<Diagnostic>)>) {
        let prepare_future = |(uri, diags)| self.client.publish_diagnostics(uri, diags, None);

        let futures = diagnostics.into_iter().map(prepare_future);
        join_all(futures).await;
    }
}
