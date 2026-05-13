pub use miette::{Diagnostic as MietteDiagnostic, Report, Severity, SourceSpan};
pub use glyim_span::{Span, MultiSpan};

use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ErrorCode { pub category: ErrorCategory, pub number: u16 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Lex, Parse, NameResolution, Type, Lifetime, Borrow, Comptime, Io, Internal,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cat = match self.category {
            ErrorCategory::Lex => "E", ErrorCategory::Parse => "P",
            ErrorCategory::NameResolution => "N", ErrorCategory::Type => "T",
            ErrorCategory::Lifetime => "L", ErrorCategory::Borrow => "B",
            ErrorCategory::Comptime => "C", ErrorCategory::Io => "I",
            ErrorCategory::Internal => "X",
        };
        write!(f, "{}{:04}", cat, self.number)
    }
}

#[derive(Clone, Debug)]
pub struct GlyimDiagnostic {
    pub code: ErrorCode,
    pub severity: DiagSeverity,
    pub message: String,
    pub span: MultiSpan,
    pub sub_diagnostics: Vec<SubDiagnostic>,
    pub suggestions: Vec<Suggestion>,
    pub source_code: Option<Arc<str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagSeverity { Error, Warning, Note, Help }

impl From<DiagSeverity> for miette::Severity {
    fn from(s: DiagSeverity) -> Self {
        match s {
            DiagSeverity::Error => miette::Severity::Error,
            DiagSeverity::Warning => miette::Severity::Warning,
            DiagSeverity::Note | DiagSeverity::Help => miette::Severity::Advice,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SubDiagnostic { pub severity: DiagSeverity, pub message: String, pub span: Option<MultiSpan> }

#[derive(Clone, Debug)]
pub struct Suggestion { pub message: String, pub replacements: Vec<(Span, String)>, pub applicability: Applicability }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability { MachineApplicable, MaybeIncorrect, HasPlaceholders, Unspecified }

impl fmt::Display for GlyimDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "[{}] {}", self.code, self.message) }
}

impl std::error::Error for GlyimDiagnostic {}

impl MietteDiagnostic for GlyimDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> { Some(Box::new(self.code)) }
    fn severity(&self) -> Option<miette::Severity> { Some(self.severity.into()) }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        let primary = miette::LabeledSpan::new_with_span(Some(self.message.clone()), SourceSpan::from(self.span.primary));
        let secondary: Vec<miette::LabeledSpan> = self.span.secondary.iter()
            .map(|(span, label)| miette::LabeledSpan::new_with_span(Some(label.clone()), SourceSpan::from(*span)))
            .collect();
        let all: Vec<_> = std::iter::once(primary).chain(secondary).collect();
        Some(Box::new(all.into_iter()))
    }
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source_code.as_ref().map(|arc| arc as &dyn miette::SourceCode)
    }
    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.suggestions.first().map(|s| Box::new(s.message.clone()) as Box<dyn fmt::Display>)
    }
}

impl GlyimDiagnostic {
    pub fn new(code: ErrorCode, severity: DiagSeverity, message: impl Into<String>, span: MultiSpan) -> Self {
        Self { code, severity, message: message.into(), span, sub_diagnostics: Vec::new(), suggestions: Vec::new(), source_code: None }
    }

    pub fn with_source_code(mut self, source: Arc<str>) -> Self { self.source_code = Some(source); self }

    pub fn lex_error(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorCode { category: ErrorCategory::Lex, number: 1 }, DiagSeverity::Error, message, MultiSpan::from_span(span))
    }
    pub fn parse_error(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorCode { category: ErrorCategory::Parse, number: 1 }, DiagSeverity::Error, message, MultiSpan::from_span(span))
    }
    pub fn type_error(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorCode { category: ErrorCategory::Type, number: 1 }, DiagSeverity::Error, message, MultiSpan::from_span(span))
    }
    pub fn borrow_error(span: Span, message: impl Into<String>) -> Self {
        Self::new(ErrorCode { category: ErrorCategory::Borrow, number: 1 }, DiagSeverity::Error, message, MultiSpan::from_span(span))
    }
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode { category: ErrorCategory::Internal, number: 0 }, DiagSeverity::Error, message, MultiSpan::from_span(Span::DUMMY))
    }

    pub fn with_sub(mut self, sub: SubDiagnostic) -> Self { self.sub_diagnostics.push(sub); self }
    pub fn with_suggestion(mut self, sug: Suggestion) -> Self { self.suggestions.push(sug); self }
    pub fn is_error(&self) -> bool { matches!(self.severity, DiagSeverity::Error) }
}

pub type CompResult<T> = Result<T, Vec<GlyimDiagnostic>>;

pub struct DiagSink {
    diagnostics: Vec<GlyimDiagnostic>,
    error_count: usize,
    suppressed_count: usize,
    error_limit: usize,
    on_emit: Option<Box<dyn FnMut(&GlyimDiagnostic)>>,
}

impl DiagSink {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            error_count: 0,
            suppressed_count: 0,
            error_limit: 50,
            on_emit: Some(Box::new(|diag| {
                match diag.severity {
                    DiagSeverity::Error => tracing::warn!("[{}] {}", diag.code, diag.message),
                    DiagSeverity::Warning => tracing::info!("[{}] {}", diag.code, diag.message),
                    DiagSeverity::Note | DiagSeverity::Help => {}
                }
            })),
        }
    }

    pub fn with_error_limit(limit: usize) -> Self { Self { error_limit: limit, ..Self::new() } }

    pub fn with_on_emit(on_emit: Option<Box<dyn FnMut(&GlyimDiagnostic)>>) -> Self {
        Self { on_emit, ..Self::new() }
    }

    pub fn emit(&mut self, diag: GlyimDiagnostic) {
        if diag.is_error() {
            if self.error_count >= self.error_limit { self.suppressed_count += 1; return; }
            self.error_count += 1;
        }
        if let Some(cb) = &mut self.on_emit { cb(&diag); }
        self.diagnostics.push(diag);
    }

    pub fn has_errors(&self) -> bool { self.error_count > 0 }
    pub fn diagnostics(&self) -> &[GlyimDiagnostic] { &self.diagnostics }

    pub fn into_diagnostics(mut self) -> Vec<GlyimDiagnostic> {
        if self.suppressed_count > 0 {
            self.diagnostics.push(GlyimDiagnostic::internal_error(format!(
                "Too many errors emitted; stopping now. ({} errors suppressed)", self.suppressed_count
            )));
        }
        self.diagnostics
    }
}

impl Default for DiagSink { fn default() -> Self { Self::new() } }

impl Extend<GlyimDiagnostic> for DiagSink {
    fn extend<T: IntoIterator<Item = GlyimDiagnostic>>(&mut self, iter: T) { for d in iter { self.emit(d); } }
}
