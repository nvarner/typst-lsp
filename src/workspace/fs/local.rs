use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;
use typst::foundations::Bytes;
use typst::syntax::Source;
use walkdir::WalkDir;

use crate::ext::PathExt;
use crate::workspace::package::manager::PackageManager;

use super::{FsError, FsResult, ReadProvider, SourceSearcher, WriteProvider};

/// Implements the Typst filesystem on the local filesystem, mapping Typst files to local files, and
/// providing conversions using [`Path`]s as an intermediate.
///
/// In this context, a "path" refers to an absolute path in the local filesystem. Paths in the Typst
/// filesystem are absolute, relative to either the project or some package. They use the same type,
/// but are meaningless when interpreted as local paths without accounting for the project or
/// package root. So, for consistency, we avoid using these Typst paths and prefer filesystem paths.
#[derive(Debug, Default)]
pub struct LocalFs {}

impl ReadProvider for LocalFs {
    fn read_bytes(&self, uri: &Url, _: &PackageManager) -> FsResult<Bytes> {
        let path = Self::uri_to_path(uri)?;
        Self::read_path_raw(&path).map(Bytes::from)
    }

    fn read_source(&self, uri: &Url, package_manager: &PackageManager) -> FsResult<Source> {
        let path = Self::uri_to_path(uri)?;

        if !path.is_typst() {
            return Err(FsError::NotSource);
        }

        let text = Self::read_path_string(&path)?;
        let full_id = package_manager.full_id(uri)?;
        Ok(Source::new(full_id.into(), text))
    }
}

impl WriteProvider for LocalFs {
    fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        let path = Self::uri_to_path(uri)?;
        Self::write_path_raw(&path, data)
    }
}

impl SourceSearcher for LocalFs {
    fn search_sources(&self, root: &Url) -> FsResult<Vec<Url>> {
        let path = Self::uri_to_path(root)?;

        let sources = WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .filter(|file| file.path().is_typst())
            .map(|file| {
                LocalFs::path_to_uri(file.path())
                    .expect("path should be absolute since walkdir was given an absolute path")
            })
            .collect();

        Ok(sources)
    }
}

impl LocalFs {
    pub fn uri_to_path(uri: &Url) -> Result<PathBuf, UriToFsPathError> {
        Self::verify_local(uri)?
            .to_file_path()
            .map_err(|()| UriToFsPathError::Conversion)
    }

    fn verify_local(uri: &Url) -> Result<&Url, UriToFsPathError> {
        if uri.scheme() == "file" {
            Ok(uri)
        } else {
            Err(UriToFsPathError::SchemeIsNotFile)
        }
    }

    /// Convert a path to its corresponding `file://` URI. Returns `Err` if the path is not
    /// absolute.
    pub fn path_to_uri(path: impl AsRef<Path>) -> Result<Url, FsPathToUriError> {
        Url::from_file_path(path).map_err(|()| FsPathToUriError::NotAbsolute)
    }

    /// Regular read from filesystem, returning a [`FileResult`] on failure
    pub fn read_path_raw(path: &Path) -> FsResult<Vec<u8>> {
        fs::read(path).map_err(|err| FsError::from_local_io(err, path))
    }

    pub fn read_path_string(path: &Path) -> FsResult<String> {
        fs::read_to_string(path).map_err(|err| FsError::from_local_io(err, path))
    }

    pub fn write_path_raw(path: &Path, data: &[u8]) -> FsResult<()> {
        fs::write(path, data).map_err(|err| FsError::from_local_io(err, path))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UriToFsPathError {
    #[error("cannot convert to path since scheme of URI is not `file`")]
    SchemeIsNotFile,
    #[error("URI to path conversion error")]
    Conversion,
}

#[derive(thiserror::Error, Debug)]
pub enum FsPathToUriError {
    #[error("cannot convert to URI since path is not absolute")]
    NotAbsolute,
}

#[cfg(test)]
mod test {
    use temp_dir::TempDir;

    use crate::workspace::package::external::manager::ExternalPackageManager;

    use super::*;

    #[test]
    fn read() {
        const BASIC_SOURCE: &str = "hello, world!";
        const BASIC_SOURCE_PATH: &str = "basic.typ";

        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.child(BASIC_SOURCE_PATH), BASIC_SOURCE).unwrap();

        let local_fs = LocalFs::default();

        let root_uri = LocalFs::path_to_uri(temp_dir.path()).unwrap();
        let package_manager = PackageManager::new(vec![root_uri], ExternalPackageManager::new());

        let basic_path = temp_dir.child(BASIC_SOURCE_PATH);
        let basic_uri = LocalFs::path_to_uri(basic_path).unwrap();

        let basic_source = local_fs
            .read_source(&basic_uri, &package_manager)
            .expect("error reading source");
        let basic_bytes = local_fs
            .read_bytes(&basic_uri, &package_manager)
            .expect("error reading bytes");

        assert_eq!(
            BASIC_SOURCE,
            basic_source.text(),
            "file contents were unexpected when reading as source"
        );
        assert_eq!(
            BASIC_SOURCE.as_bytes(),
            basic_bytes.as_slice(),
            "file contents were unexpected when reading as bytes"
        );
    }
}

#[cfg(test)]
mod test_conversions {
    use std::ffi::OsString;

    use super::*;

    #[test]
    fn path_uri_path() {
        for path in valid_paths() {
            let uri = LocalFs::path_to_uri(&path).unwrap();
            let converted_path = LocalFs::uri_to_path(&uri).unwrap();

            assert_eq!(path, converted_path, "path changed via conversion to URI");
        }
    }

    /// UNIX filenames are essentially arbitrary byte strings which may contain anything other than
    /// `\0` and `/`.
    ///
    /// ## See also
    /// - https://stackoverflow.com/a/31976060
    #[cfg(unix)]
    fn valid_chars() -> impl Iterator<Item = OsString> {
        use std::os::unix::ffi::OsStringExt;

        (u8::MIN..=u8::MAX)
            .filter(|c: &u8| *c != b'\0' && *c != b'/')
            .map(|c| OsString::from_vec(vec![c]))
    }

    /// Windows filenames have more restrictions than UNIX. They seem to be treated as Unicode
    /// strings. Unprintable characters are forbidden, and there is a list of printable forbidden
    /// characters, such as `<`, `"`, and `*`.
    ///
    /// Certain filenames are also prohibited, but should not affect this test.
    ///
    /// ## See also
    /// - https://stackoverflow.com/a/31976060
    /// - https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file
    #[cfg(windows)]
    fn valid_chars() -> impl Iterator<Item = OsString> {
        let forbidden_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
        let ascii = (32..=127)
            .map(|c: u8| c as char)
            .filter(move |c| !forbidden_chars.contains(c));

        let utf8 = ['∀', '⨅', '我', 'あ', '한', '🎦'];

        ascii.chain(utf8).map(|c| OsString::from(c.to_string()))
    }

    #[cfg(not(any(unix, windows)))]
    fn valid_chars() -> impl Iterator<Item = OsString> {
        compile_error!("don't know valid filename chars on this OS!")
    }

    #[cfg(unix)]
    fn path_prefix() -> &'static Path {
        Path::new("/some/example/path/for/testing/")
    }

    #[cfg(windows)]
    fn path_prefix() -> &'static Path {
        Path::new("C:\\some\\example\\path\\for\\testing\\")
    }

    #[cfg(not(any(unix, windows)))]
    fn path_prefix() -> &'static Path {
        compile_error!("don't know a path prefix on this OS!")
    }

    fn valid_paths() -> impl Iterator<Item = PathBuf> {
        valid_chars().map(|c| path_prefix().join(c))
    }
}
