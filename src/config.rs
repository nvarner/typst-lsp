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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputRoot {
    Source,
    Workspace,
    Absolute,
}

impl Default for OutputRoot {
    fn default() -> Self {
        Self::Source
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub export_pdf: ExportPdfMode,
    pub output_root: OutputRoot,
    pub output_path: String,
}
