use std::collections::HashMap;

use tower_lsp::lsp_types::Url;

use super::resource::Resource;

pub struct ResourceManager {
    resources: HashMap<Url, Resource>,
}

impl ResourceManager {
    pub fn get_resource_by_uri(&self, uri: &Url) -> Option<&Resource> {
        self.resources.get(uri)
    }
}
