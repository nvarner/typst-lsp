#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPdfMode {
    Never,
    OnSave,
    OnType,
}

impl Default for ExportPdfMode {
    fn default() -> Self {
        Self::OnSave
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub export_pdf: ExportPdfMode,
    pub out_dir: String,
}
