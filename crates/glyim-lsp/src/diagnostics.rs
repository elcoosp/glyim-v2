use glyim_span::FileId;
use glyim_diag::GlyimDiagnostic;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

pub fn convert_diagnostics(
    _file_id: FileId,
    _source_map: &crate::database::SourceMap,
    diags: &[GlyimDiagnostic],
) -> Vec<Diagnostic> {
    diags.iter().map(|d| Diagnostic {
        range: Range::default(),
        severity: Some(match d.severity {
            glyim_diag::DiagSeverity::Error => DiagnosticSeverity::ERROR,
            glyim_diag::DiagSeverity::Warning => DiagnosticSeverity::WARNING,
            _ => DiagnosticSeverity::INFORMATION,
        }),
        source: Some("glyim".to_string()),
        message: d.message.clone(),
        ..Default::default()
    }).collect()
}
