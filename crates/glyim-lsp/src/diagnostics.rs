use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

pub fn convert_diagnostics(
    _file_id: FileId,
    source_map: &crate::database::SourceMap,
    diags: &[GlyimDiagnostic],
) -> Vec<Diagnostic> {
    diags
        .iter()
        .map(|d| {
            let range = if let Some(((start_line, start_col), (end_line, end_col))) = source_map
                .span_to_position(d.span.primary.lo.to_usize(), d.span.primary.hi.to_usize())
            {
                Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                }
            } else {
                Range::default()
            };
            Diagnostic {
                range,
                severity: Some(match d.severity {
                    glyim_diag::DiagSeverity::Error => DiagnosticSeverity::ERROR,
                    glyim_diag::DiagSeverity::Warning => DiagnosticSeverity::WARNING,
                    _ => DiagnosticSeverity::INFORMATION,
                }),
                source: Some("glyim".to_string()),
                message: d.message.clone(),
                ..Default::default()
            }
        })
        .collect()
}
