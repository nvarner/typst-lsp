use std::collections::hash_map::Entry;
use std::collections::HashMap;

use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};

use super::resource::Resource;

#[derive(Debug, Default)]
pub struct ResourceManager {
    resources: HashMap<Url, Resource>,
}

impl ResourceManager {
    pub fn clear(&mut self) {
        self.resources.clear();
    }

    pub fn get_or_insert_resource(&mut self, uri: Url) -> FileResult<&Resource> {
        match self.resources.entry(uri.clone()) {
            Entry::Vacant(entry) => {
                // TODO: ideally, we do this through the LSP client instead, and watch the file to
                // avoid caching old data
                let resource = Resource::read_file(&uri).map_err(|_| FileError::Other)?;
                Ok(entry.insert(resource))
            }
            Entry::Occupied(entry) => Ok(entry.into_mut()),
        }
    }
}
