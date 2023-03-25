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

pub struct Config {
    pub export_pdf: ExportPdfMode,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}
