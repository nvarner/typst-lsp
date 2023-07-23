use std::mem;

use elsa::sync::FrozenVec;
use indexmap::IndexSet;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use tower_lsp::lsp_types::Url;
use tracing::{info, trace};
use typst::diag::FileResult;
use typst::syntax::SourceId;
use walkdir::WalkDir;

use crate::lsp_typst_boundary::{lsp_to_typst, typst_to_lsp};

use super::source::Source;

/// Provides access to [`Source`] documents via [`SourceId`]s and [`Url`]s
///
/// A document can be open or closed. "Open" and "closed" correspond to the document's reported
/// state in the LSP client.
#[derive(Default)]
pub struct SourceManager {
    ids: RwLock<IndexSet<Url>>,
    sources: FrozenVec<Box<InnerSource>>,
}

impl SourceManager {
    /// Get the URIs of all sources which have been seen
    pub fn get_uris(&self) -> Vec<Url> {
        self.ids.read().iter().cloned().collect()
    }

    /// Get a [`Source`] by its URI, caching it and adding it to the `SourceManager` if needed
    pub fn get_source_by_uri(&self, uri: Url) -> FileResult<&Source> {
        self.get_all_by_uri(uri).map(|(source, _)| source)
    }

    pub fn get_mut_source_by_uri(&mut self, uri: Url) -> FileResult<&mut Source> {
        self.get_mut_all_by_uri(uri).map(|(source, _)| source)
    }

    /// Get a document's [`SourceId`] by its URI, caching it and adding it to the `SourceManager` if needed
    pub fn get_id_by_uri(&self, uri: Url) -> FileResult<SourceId> {
        self.get_all_by_uri(uri).map(|(_, id)| id)
    }

    pub fn get_source_by_id(&self, id: SourceId) -> FileResult<&Source> {
        self.get_inner_source(id).get_or_init_source(id)
    }

    /// Open a document, adding it to the `SourceManager` if needed
    #[tracing::instrument(skip_all, fields(%uri))]
    pub fn open(&mut self, uri: &Url, text: String) -> anyhow::Result<()> {
        Self::with_id(self.ids.get_mut(), uri.clone(), |id, is_new| {
            let inner = InnerSource::Open(Source::new(id, uri, text)?);
            if is_new {
                info!(id = id.as_u16(), "new source opened");
                self.sources.push(Box::new(inner));
            } else {
                info!(id = id.as_u16(), "existing source opened");
                **self
                    .sources
                    .as_mut()
                    .get_mut(usize::from(id.as_u16()))
                    .unwrap() = inner;
            }
            Ok(())
        })
    }

    /// Close a document
    #[tracing::instrument(skip_all, fields(%uri))]
    pub fn close(&mut self, uri: Url) {
        if let Some(id) = self.get_id_by_known_uri(&uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::Open(source) = inner_source {
                info!(id = id.as_u16(), "open source closed");
                let source = mem::replace(source, Source::new_detached());
                *inner_source = InnerSource::closed(source, uri);
            }
        }
    }

    /// Invalidate a document if it is cached
    #[tracing::instrument(skip_all, fields(%uri))]
    pub fn invalidate(&mut self, uri: &Url) {
        if let Some(id) = self.get_id_by_known_uri(uri) {
            let inner_source = self.get_mut_inner_source(id);
            if let InnerSource::Closed(cell, _) = inner_source {
                info!(id = id.as_u16(), "close source invalidated");
                cell.take();
            }
        }
    }

    /// Add all Typst files in `workspace` to the `SourceManager`, caching them as needed
    #[tracing::instrument(skip_all, fields(%workspace))]
    pub fn register_workspace_files(&self, workspace: &Url) -> anyhow::Result<()> {
        let workspace_path = lsp_to_typst::uri_to_path(workspace)?;

        let walker = WalkDir::new(workspace_path).into_iter();
        let typst_file_uris = walker
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|file| file.path().extension().map_or(false, |ext| ext == "typ"))
            .map(|file| typst_to_lsp::path_to_uri(file.path()))
            .filter_map(Result::ok);

        for uri in typst_file_uris {
            trace!(%uri, "registering file");
            self.get_id_by_uri(uri)?;
        }

        Ok(())
    }

    /// Get a [`Source`] and its [`SourceId`] by its URI, caching it and adding it to the
    /// `SourceManager` if needed
    #[tracing::instrument(skip_all, fields(%uri))]
    pub fn get_all_by_uri(&self, uri: Url) -> FileResult<(&Source, SourceId)> {
        Self::with_id(&mut self.ids.write(), uri.clone(), |id, is_new| {
            let source = if is_new {
                self.sources
                    .push_get(Box::new(InnerSource::closed(
                        Source::read_from_file(id, &uri)?,
                        uri,
                    )))
                    .get_source()
                    .unwrap()
            } else {
                self.sources
                    .get(id.as_u16().into())
                    .unwrap()
                    .get_or_init_source(id)?
            };
            Ok((source, id))
        })
    }

    /// Get a [`Source`] and its [`SourceId`] by its URI, caching it and adding it to the
    /// `SourceManager` if needed
    #[tracing::instrument(skip_all, fields(%uri))]
    pub fn get_mut_all_by_uri(&mut self, uri: Url) -> FileResult<(&mut Source, SourceId)> {
        Self::with_id(self.ids.get_mut(), uri.clone(), |id, is_new| {
            let source =
                if is_new {
                    let sources = self.sources.as_mut();
                    sources.push(Box::new(InnerSource::closed(
                        Source::read_from_file(id, &uri)?,
                        uri,
                    )));
                    sources.last_mut()
                .expect("`sources` should be nonempty since we just pushed to it")
                .get_mut_source()
                 .expect("last element should have a source since we just pushed one with a source")
                } else {
                    let inner = self
                        .sources
                        .as_mut()
                        .get_mut(usize::from(id.as_u16()))
                        .unwrap();
                    inner.get_or_init_source(id)?;
                    inner
                        .get_mut_source()
                        .expect("cell should have just been initialized")
                };
            Ok((source, id))
        })
    }

    fn get_id_by_known_uri(&self, uri: &Url) -> Option<SourceId> {
        self.ids
            .read()
            .get_index_of(uri)
            .map(|id| SourceId::from_u16(id as u16))
    }

    fn get_inner_source(&self, id: SourceId) -> &InnerSource {
        // We treat all `SourceId`s as valid
        self.sources.get(id.as_u16() as usize).unwrap()
    }

    fn get_mut_inner_source(&mut self, id: SourceId) -> &mut InnerSource {
        // We treat all `SourceId`s as valid
        self.sources.as_mut().get_mut(id.as_u16() as usize).unwrap()
    }

    /// Retrieve or generate the ID for `uri` and run a function on it.
    ///
    /// `op` is required to add an entry to [`Self::sources`] or return [`Result::Err`]
    /// when the bool argument is true, meaning the URI was not yet cached.
    /// Otherwise the IndexSet and sources Vec would become out of sync.
    fn with_id<T, E>(
        ids: &mut IndexSet<Url>,
        uri: Url,
        op: impl FnOnce(SourceId, bool) -> Result<T, E>,
    ) -> Result<T, E> {
        let (index, uri_is_new) = ids.insert_full(uri);
        let id = SourceId::from_u16(index.try_into().unwrap());
        op(id, uri_is_new).map_err(|err| {
            if uri_is_new {
                ids.pop();
            }
            err
        })
    }
}

#[derive(Debug)]
enum InnerSource {
    Open(Source),
    Closed(OnceCell<Source>, Url),
}

impl InnerSource {
    pub fn closed(source: Source, uri: Url) -> Self {
        Self::Closed(OnceCell::with_value(source), uri)
    }

    pub fn get_source(&self) -> Option<&Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell, _) => cell.get(),
        }
    }

    pub fn get_or_init_source(&self, id: SourceId) -> FileResult<&Source> {
        match self {
            Self::Open(source) => Ok(source),
            Self::Closed(cell, uri) => cell.get_or_try_init(|| Source::read_from_file(id, uri)),
        }
    }

    pub fn get_mut_source(&mut self) -> Option<&mut Source> {
        match self {
            Self::Open(source) => Some(source),
            Self::Closed(cell, _) => cell.get_mut(),
        }
    }
}
