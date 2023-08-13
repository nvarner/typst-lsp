use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::str::Utf8Error;

use itertools::{EitherOrBoth, Itertools};
use percent_encoding::{percent_decode_str, PercentDecode};
use tower_lsp::lsp_types::Url;
use tower_lsp::lsp_types::{
    InitializeParams, Position, PositionEncodingKind, SemanticTokensClientCapabilities,
};
use typst::syntax::FileId;

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
            PositionEncoding::Utf16 => self.chars().map(char::len_utf16).sum(),
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

    /// Unless this URL is cannot-be-a-base, returns the path segments, percent decoded into UTF-8
    /// strings, if possible.
    fn path_segments_decoded(&self) -> UriResult<Vec<Cow<str>>>;

    /// Get a new URI, replacing the existing file extension with the given extension, if there is a
    /// file extension to replace.
    fn with_extension(self, extension: &str) -> UriResult<Url>;
}

impl UrlExt for Url {
    fn join_rooted(mut self, path: &Path) -> Result<Url, UriError> {
        let mut added_len: usize = 0;
        let mut segments = self
            .path_segments_mut()
            .map_err(|()| UriError::CannotBeABase)?;

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

        let root = self.path_segments_decoded()?;
        let sub = sub_uri.path_segments_decoded()?;

        let root_iter = root.iter().map(Cow::as_ref);
        let sub_iter = sub.iter().map(Cow::as_ref);

        let relative_path: PathBuf = root_iter
            .zip_longest(sub_iter)
            .skip_while(EitherOrBoth::is_both)
            .map(|x| x.right().ok_or(UriError::PathEscapesRoot))
            .try_collect()?;

        Ok(relative_path.push_front(Path::root()))
    }

    fn path_segments_decoded(&self) -> UriResult<Vec<Cow<str>>> {
        self.path_segments()
            .ok_or(UriError::CannotBeABase)
            .and_then(|segments| {
                segments
                    .map(percent_decode_str)
                    .map(PercentDecode::decode_utf8)
                    .try_collect()
                    .map_err(UriError::from)
            })
    }

    fn with_extension(mut self, extension: &str) -> UriResult<Url> {
        let filename = self
            .path_segments()
            .ok_or(UriError::CannotBeABase)?
            .last()
            .unwrap_or("");
        let filename_decoded = percent_decode_str(filename).decode_utf8()?;

        let new_filename_path = Path::new(filename_decoded.as_ref()).with_extension(extension);
        let new_filename = new_filename_path
            .to_str()
            .expect("the path should come from `filename` and `extension`; both are valid UTF-8");

        self.path_segments_mut()
            .map_err(|()| UriError::CannotBeABase)?
            .pop()
            .push(new_filename);

        Ok(self)
    }
}

pub type UriResult<T> = Result<T, UriError>;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum UriError {
    #[error("URI cannot be a base")]
    CannotBeABase,
    #[error("path escapes root")]
    PathEscapesRoot,
    #[error("could not decode")]
    Encoding(#[from] Utf8Error),
}

#[cfg(test)]
mod uri_test {
    use super::*;

    #[test]
    fn join_rooted() {
        let url = Url::parse("file:///path/to").unwrap();
        let path = Path::new("/file.typ");

        let joined = url.join_rooted(path).unwrap();

        let expected = Url::parse("file:///path/to/file.typ").unwrap();
        assert_eq!(expected, joined);
    }

    #[test]
    fn join_rooted_utf8() {
        let url = Url::parse("file:///path/%E6%B1%89%E5%AD%97/to").unwrap();
        let path = Path::new("/汉字.typ");

        let joined = url.join_rooted(path).unwrap();

        let expected =
            Url::parse("file:///path/%E6%B1%89%E5%AD%97/to/%E6%B1%89%E5%AD%97.typ").unwrap();
        assert_eq!(expected, joined);
    }

    #[test]
    fn join_rooted_escape() {
        let url = Url::parse("file:///path/to").unwrap();
        let escapee = Path::new("/../../etc/passwd");

        let error = url.join_rooted(escapee).unwrap_err();

        assert_eq!(UriError::PathEscapesRoot, error);
    }

    #[test]
    fn make_relative_rooted() {
        let base_url = Url::parse("file:///path").unwrap();
        let sub_url = Url::parse("file:///path/to/file.typ").unwrap();

        let relative = base_url.make_relative_rooted(&sub_url).unwrap();

        assert_eq!(Path::new("/to/file.typ"), &relative);
    }

    #[test]
    fn make_relative_rooted_utf8() {
        let base_url = Url::parse("file:///path/%E6%B1%89%E5%AD%97/dir").unwrap();
        let sub_url =
            Url::parse("file:///path/%E6%B1%89%E5%AD%97/dir/to/%E6%B1%89%E5%AD%97.typ").unwrap();

        let relative = base_url.make_relative_rooted(&sub_url).unwrap();

        assert_eq!(Path::new("/to/汉字.typ"), &relative);
    }

    #[test]
    fn path_segments_decode() {
        let url = Url::parse("file:///path/to/file.typ").unwrap();

        let segments = url.path_segments_decoded().unwrap();

        assert_eq!(
            vec!["path", "to", "file.typ"],
            segments.iter().map(Cow::as_ref).collect_vec()
        )
    }

    #[test]
    fn path_segments_decode_utf8() {
        let url = Url::parse("file:///path/to/file/%E6%B1%89%E5%AD%97.typ").unwrap();

        let segments = url.path_segments_decoded().unwrap();

        assert_eq!(
            vec!["path", "to", "file", "汉字.typ"],
            segments.iter().map(Cow::as_ref).collect_vec()
        )
    }

    #[test]
    fn with_extension() {
        let url = Url::parse("file:///path/to/file.typ").unwrap();

        let pdf_url = url.with_extension("pdf").unwrap();

        let expected = Url::parse("file:///path/to/file.pdf").unwrap();
        assert_eq!(expected, pdf_url);
    }

    #[test]
    fn with_extension_utf8() {
        let url = Url::parse("file:///path/to/file/%E6%B1%89%E5%AD%97.typ").unwrap();

        let pdf_url = url.with_extension("pdf").unwrap();

        let expected = Url::parse("file:///path/to/file/%E6%B1%89%E5%AD%97.pdf").unwrap();
        assert_eq!(expected, pdf_url);
    }
}
