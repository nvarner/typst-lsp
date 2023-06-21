use std::fs::File;
use std::io::{self, Read};

use tower_lsp::lsp_types::Url;
use typst::util::Buffer;

/// Files used by Typst source code, like fonts or images
#[derive(Debug, Clone)]
pub struct Resource {
    // This is driven by the interface of Typst's `World` trait and `Font` struct
    buffer: Buffer,
}

impl Resource {
    pub fn read_file(uri: &Url) -> io::Result<Self> {
        let buffer = Self::read_file_to_buffer(uri)?;
        Ok(Self { buffer })
    }

    fn read_file_to_buffer(uri: &Url) -> io::Result<Buffer> {
        let path = uri.to_file_path().map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("could not get path for URI {uri}"),
            )
        })?;
        let mut file = File::open(path)?;

        let mut buffer_data = Vec::new();
        file.read_to_end(&mut buffer_data)?;

        let buffer = Buffer::from(buffer_data);

        Ok(buffer)
    }
}

impl From<Resource> for Buffer {
    fn from(lsp_resource: Resource) -> Self {
        lsp_resource.buffer
    }
}
