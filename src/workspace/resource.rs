use typst::util::Buffer;

/// Files used by Typst source code, like fonts or images
#[derive(Debug, Clone)]
pub struct Resource {
    // This is driven by the interface of Typst's `World` trait
    buffer: Buffer,
}

impl From<&Resource> for Buffer {
    fn from(lsp_resource: &Resource) -> Self {
        lsp_resource.buffer.clone()
    }
}
