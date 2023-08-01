use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use itertools::{EitherOrBoth, Itertools};
use tower_lsp::lsp_types::Url;
use tower_lsp::lsp_types::{
    InitializeParams, Position, PositionEncodingKind, SemanticTokensClientCapabilities,
};
use typst::file::FileId;
use typst::util::StrExt as TypstStrExt;

use crate::config::PositionEncoding;
use crate::workspace::fs::local::LocalFs;
use crate::workspace::package::{FullFileId, PackageId};

pub trait InitializeParamsExt {
    fn position_encodings(&self) -> &[PositionEncodingKind];
    fn supports_config_change_registration(&self) -> bool;
    fn semantic_tokens_capabilities(&self) -> Option<&SemanticTokensClientCapabilities>;
    fn supports_semantic_tokens_dynamic_registration(&self) -> bool;
    fn root_uris(&self) -> Vec<Url>;
}

static DEFAULT_ENCODING: [PositionEncodingKind; 1] = [PositionEncodingKind::UTF16];

impl InitializeParamsExt for InitializeParams {
    fn position_encodings(&self) -> &[PositionEncodingKind] {
        self.capabilities
            .general
            .as_ref()
            .and_then(|general| general.position_encodings.as_ref())
            .map(|encodings| encodings.as_slice())
            .unwrap_or(&DEFAULT_ENCODING)
    }

    fn supports_config_change_registration(&self) -> bool {
        self.capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.configuration)
            .unwrap_or(false)
    }

    fn semantic_tokens_capabilities(&self) -> Option<&SemanticTokensClientCapabilities> {
        self.capabilities
            .text_document
            .as_ref()?
            .semantic_tokens
            .as_ref()
    }

    fn supports_semantic_tokens_dynamic_registration(&self) -> bool {
        self.semantic_tokens_capabilities()
            .and_then(|semantic_tokens| semantic_tokens.dynamic_registration)
            .unwrap_or(false)
    }

    #[allow(deprecated)] // `self.root_path` is marked as deprecated
    fn root_uris(&self) -> Vec<Url> {
        match self.workspace_folders.as_ref() {
            Some(roots) => roots.iter().map(|root| &root.uri).cloned().collect(),
            None => {
                let root_uri = || self.root_uri.as_ref().cloned();
                let root_path = || LocalFs::path_to_uri(self.root_path.as_ref()?).ok();

                root_uri().or_else(root_path).into_iter().collect()
            }
        }
    }
}

pub trait StrExt {
    fn encoded_len(&self, encoding: PositionEncoding) -> usize;
}

impl StrExt for str {
    fn encoded_len(&self, encoding: PositionEncoding) -> usize {
        match encoding {
            PositionEncoding::Utf8 => self.len(),
            PositionEncoding::Utf16 => self.len_utf16(),
        }
    }
}

pub trait PathExt {
    /// Creates a [`PathBuf`] with `self` adjoined to `prefix`. See [`PathBuf::push`] for semantics.
    fn push_front(&self, prefix: impl AsRef<Path>) -> PathBuf;

    fn root() -> &'static Path;
    fn is_typst(&self) -> bool;
}

impl PathExt for Path {
    fn push_front(&self, prefix: impl AsRef<Path>) -> PathBuf {
        prefix.as_ref().join(self)
    }

    fn root() -> &'static Path {
        Path::new("/")
    }

    fn is_typst(&self) -> bool {
        self.extension().map_or(false, |ext| ext == "typ")
    }
}

pub trait FileIdExt {
    fn with_extension(self, extension: impl AsRef<OsStr>) -> Self;
    fn fill(self, current: PackageId) -> FullFileId;
}

impl FileIdExt for FileId {
    fn with_extension(self, extension: impl AsRef<OsStr>) -> Self {
        let path = self.path().with_extension(extension);
        Self::new(self.package().cloned(), &path)
    }

    fn fill(self, current: PackageId) -> FullFileId {
        let package = self
            .package()
            .cloned()
            .map(PackageId::new_external)
            .unwrap_or(current);
        FullFileId::new(package, self.path().to_owned())
    }
}

pub trait PositionExt {
    fn delta(&self, to: &Self) -> PositionDelta;
}

impl PositionExt for Position {
    /// Calculates the delta from `self` to `to`. This is in the `SemanticToken` sense, so the
    /// delta's `character` is relative to `self`'s `character` iff `self` and `to` are on the same
    /// line. Otherwise, it's relative to the start of the line `to` is on.
    fn delta(&self, to: &Self) -> PositionDelta {
        let line_delta = to.line - self.line;
        let char_delta = if line_delta == 0 {
            to.character - self.character
        } else {
            to.character
        };

        PositionDelta {
            delta_line: line_delta,
            delta_start: char_delta,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Default)]
pub struct PositionDelta {
    pub delta_line: u32,
    pub delta_start: u32,
}

pub trait UrlExt {
    /// Joins the path to the URI, treating the URI as if it was the root directory. Returns `Err`
    /// if the path leads out of the root or the URI cannot be used as a base.
    fn join_rooted(self, path: &Path) -> UriResult<Url>;

    /// Gets the relative path to the sub URI, treating this URI as if it was the root. Returns
    /// `None` if the path leads out of the root.
    fn make_relative_rooted(&self, sub_uri: &Url) -> UriResult<PathBuf>;
}

impl UrlExt for Url {
    fn join_rooted(mut self, path: &Path) -> Result<Url, UriError> {
        let mut added_len: usize = 0;
        let mut segments = self
            .path_segments_mut()
            .map_err(|()| UriError::UriCannotBeABase)?;

        for component in path.components() {
            match component {
                Component::Normal(segment) => {
                    added_len += 1;
                    segments.push(segment.to_str().expect("all package paths should be UTF-8"));
                }
                Component::ParentDir => {
                    added_len.checked_sub(1).ok_or(UriError::PathEscapesRoot)?;
                    segments.pop();
                }
                Component::CurDir => (),
                // should occur only at the start, when the URI is already root, so nothing to do
                Component::Prefix(_) | Component::RootDir => (),
            }
        }

        // must drop before return to ensure its `Drop` doesn't use borrowed `self` after move
        drop(segments);

        Ok(self)
    }

    fn make_relative_rooted(&self, sub_uri: &Url) -> UriResult<PathBuf> {
        if self.scheme() != sub_uri.scheme() || self.authority() != sub_uri.authority() {
            return Err(UriError::PathEscapesRoot);
        }

        let root = self.path_segments().ok_or(UriError::UriCannotBeABase)?;
        let sub = sub_uri.path_segments().ok_or(UriError::UriCannotBeABase)?;

        let relative_path: PathBuf = root
            .zip_longest(sub)
            .skip_while(EitherOrBoth::is_both)
            .map(|x| x.right().ok_or(UriError::PathEscapesRoot))
            .try_collect()?;

        Ok(relative_path.push_front(Path::root()))
    }
}

pub type UriResult<T> = Result<T, UriError>;

#[derive(thiserror::Error, Debug)]
pub enum UriError {
    #[error("URI cannot be a base")]
    UriCannotBeABase,
    #[error("path escapes root")]
    PathEscapesRoot,
}
