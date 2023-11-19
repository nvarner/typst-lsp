//! Holds types relating to the LSP concept of a "workspace". That is, the directories a user has
//! open in their editor, the files in them, the files they're currently editing, and so on.
//!
//! # Terminology
//! - Package: anything that can be described by described by an `Option<PackageSpec>`, i.e. both
//!     external packages and current packages
//!     - External package: anything that can be described by a `PackageSpec`. A versioned package,
//!         likely downloaded, used as a dependency.
//!         - e.g. [`@preview/example:0.1.0`](https://github.com/typst/packages/tree/main/packages/preview/example-0.1.0)
//!     - Current package: something described by `None` as a value in `Option<PackageSpec>`. The
//!         set of files the user is actively working with, which should include a `main` source
//!         file.
//!         - e.g. a main resume source, template source, and some images
//!         - e.g. several chapter sources and a main source combining them
//!         - e.g. several main sources, each corresponding to one homework assignment for a course
//! - Project: a single current package together with a set of external packages
//!     - e.g. a current package with course notes and external packages for tables and commutative
//!         diagrams
//! - ProjectWorld: a project and a single main source picked from the current project
//!     - e.g. the latest homework assignment as main from several homework assignment sources
//!
//! # [`FileId`] interpretation
//! To interpret a `FileId`, we need to get a package from [`FileId::package`], then, within that
//! package, take the file at the path given by [`FileId::path`].
//!
//! To interpret `FileId::package` in general, we need a unique current package and a set of
//! external packages, which is a project. To interpret `FileId::path`, all we need is a package.
//! So, packages must be able to interpret paths, and projects must be able to interpret `FileId`s.
//!
//! We can represent an interpreted `FileId` by a URI, since a URI fully specifies a file. We can
//! also go backwards, representing a URI specifying a valid file as a `FileId` together with the
//! context needed to interpret it, which is a project.

use std::collections::HashSet;
use std::path::PathBuf;

use comemo::Prehashed;
use itertools::Itertools;
use lazy_static::lazy_static;
use tower_lsp::lsp_types::{
    InitializeParams, TextDocumentContentChangeEvent, Url, WorkspaceFoldersChangeEvent,
};
use tracing::trace;
use typst::eval::{Bytes, Library};
use typst::syntax::Source;

use crate::config::{FontPaths, PositionEncoding};
use crate::ext::InitializeParamsExt;

use self::font_manager::FontManager;
use self::fs::manager::FsManager;
use self::fs::{FsResult, KnownUriProvider, ReadProvider, WriteProvider};
use self::package::external::manager::ExternalPackageManager;
use self::package::manager::PackageManager;
use self::package::{FullFileId, Package};

pub mod font_manager;
pub mod fs;
pub mod package;
pub mod project;
pub mod world;

lazy_static! {
    pub static ref TYPST_STDLIB: Prehashed<Library> = Prehashed::new(typst_library::build());
}

#[derive(Debug)]
pub struct Workspace {
    fs: FsManager,
    fonts: FontManager,
    packages: PackageManager,
}

impl Workspace {
    pub fn new(params: &InitializeParams) -> Self {
        let root_paths = params.root_uris();

        Self {
            fs: FsManager::default(),
            fonts: Self::create_font_manager(&[]),
            packages: PackageManager::new(root_paths, ExternalPackageManager::new()),
        }
    }

    pub fn font_manager(&self) -> &FontManager {
        &self.fonts
    }

    pub fn package_manager(&self) -> &PackageManager {
        &self.packages
    }

    pub fn register_files(&mut self) -> FsResult<()> {
        self.packages
            .current()
            .inspect(|package| trace!(?package, "registering files in package"))
            .map(Package::root)
            .map(|root| self.fs.register_files(root))
            .try_collect()
    }

    pub async fn uri(&self, full_id: FullFileId) -> FsResult<Url> {
        let package = self.package_manager().package(full_id.package()).await?;
        let uri = package.vpath_to_uri(full_id.vpath())?;
        Ok(uri)
    }

    pub fn full_id(&self, uri: &Url) -> FsResult<FullFileId> {
        self.packages.full_id(uri)
    }

    pub fn read_bytes(&self, uri: &Url) -> FsResult<Bytes> {
        self.fs.read_bytes(uri, &self.packages)
    }

    pub fn read_source(&self, uri: &Url) -> FsResult<Source> {
        self.fs.read_source(uri, &self.packages)
    }

    /// Write raw data to a file.
    ///
    /// This can cause cache invalidation errors if `uri` refers to a file in the cache, since the
    /// cache wouldn't know about the update. However, this is hard to fix, because we don't have
    /// `&mut self`.
    ///
    /// For example, when writing a PDF, we (effectively) have `&Workspace` after compiling via
    /// Typst, and we'd rather not lock everything just to export the PDF. However, if we allow for
    /// mutating files stored in the `Cache`, we could update a file while it is being used for a
    /// Typst compilation, which is also bad.
    pub fn write_raw(&self, uri: &Url, data: &[u8]) -> FsResult<()> {
        self.fs.write_raw(uri, data)
    }

    pub fn known_uris(&self) -> HashSet<Url> {
        self.fs.known_uris()
    }

    pub fn update_fonts(&mut self, font_paths: &FontPaths) {
        trace!("updating font paths to {font_paths:?}");
        self.fonts = Self::create_font_manager(font_paths);
    }

    pub fn open_lsp(&mut self, uri: Url, text: String) -> FsResult<()> {
        self.fs.open_lsp(uri, text, &self.packages)
    }

    pub fn close_lsp(&mut self, uri: &Url) {
        self.fs.close_lsp(uri)
    }

    pub fn edit_lsp(
        &mut self,
        uri: &Url,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
        position_encoding: PositionEncoding,
    ) {
        self.fs.edit_lsp(uri, changes, position_encoding)
    }

    pub fn new_local(&mut self, uri: Url) {
        self.fs.new_local(uri)
    }

    pub fn invalidate_local(&mut self, uri: Url) {
        self.fs.invalidate_local(uri)
    }

    pub fn delete_local(&mut self, uri: &Url) {
        self.fs.delete_local(uri)
    }

    pub fn handle_workspace_folders_change_event(
        &mut self,
        event: &WorkspaceFoldersChangeEvent,
    ) -> FsResult<()> {
        self.packages.handle_change_event(event);

        // The canonical project/id of URIs might have changed, so we need to invalidate the cache
        self.clear()?;

        Ok(())
    }

    pub fn clear(&mut self) -> FsResult<()> {
        self.fonts.clear();
        self.fs.clear();
        self.register_files()?;
        Ok(())
    }

    fn create_font_manager(font_paths: &[PathBuf]) -> FontManager {
        FontManager::builder()
            .with_system()
            .with_embedded()
            .with_font_paths(font_paths)
            .build()
    }
}
