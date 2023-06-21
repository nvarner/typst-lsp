use std::collections::hash_map::Entry;
use std::collections::HashMap;

use tower_lsp::lsp_types::Url;
use typst::diag::{FileError, FileResult};

use super::resource::Resource;

#[derive(Default)]
pub struct ResourceManager {
    resources: HashMap<Url, Option<Resource>>,
}

impl ResourceManager {
    pub fn clear(&mut self) {
        self.resources.clear();
    }

    pub fn get_by_uri(&mut self, uri: Url) -> FileResult<Resource> {
        match self.resources.entry(uri.clone()) {
            Entry::Vacant(entry) => {
                let resource = Resource::read_file(&uri).map_err(|_| FileError::Other)?;
                let resource = entry.insert(Some(resource));
                Ok(resource.as_mut().unwrap().clone())
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Some(resource) => Ok(resource.clone()),
                option @ None => {
                    let resource = Resource::read_file(&uri).map_err(|_| FileError::Other)?;
                    let resource = option.insert(resource);
                    Ok(resource.clone())
                }
            },
        }
    }

    pub fn invalidate(&mut self, uri: Url) {
        self.resources
            .entry(uri)
            .and_modify(|option| *option = None);
    }
}
