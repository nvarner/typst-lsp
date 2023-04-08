use tower_lsp::lsp_types::{InitializeParams, PositionEncodingKind};

pub trait InitializeParamsExt {
    fn position_encodings(&self) -> &[PositionEncodingKind];
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
}
