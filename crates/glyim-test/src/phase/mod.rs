pub mod analysis;
pub mod codegen_phase;
pub mod frontend;
pub mod mir_gen;

pub use analysis::AnalysisTester;
pub use codegen_phase::CodegenTester;
pub use frontend::FrontendTester;
pub use mir_gen::MirGenTester;

use glyim_diag::GlyimDiagnostic;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct CompilationTrace {
    pub lex_diagnostics: Vec<GlyimDiagnostic>,
    pub parse_diagnostics: Vec<GlyimDiagnostic>,
    pub parse_tree: Option<glyim_syntax::SyntaxNode>,
    pub def_map: Option<glyim_def_map::CrateDefMap>,
    pub def_map_diagnostics: Vec<GlyimDiagnostic>,
    pub typeck_result: Option<glyim_typeck::TypeckResult>,
    pub typeck_diagnostics: Vec<GlyimDiagnostic>,
    pub mir_bodies: Vec<Arc<glyim_mir::Body>>,
    pub lower_diagnostics: Vec<GlyimDiagnostic>,
    pub borrowck_diagnostics: Vec<GlyimDiagnostic>,
    pub optimized_bodies: Vec<Arc<glyim_mir::Body>>,
    pub codegen_output: Option<Vec<u8>>,
}

impl CompilationTrace {
    pub fn all_diagnostics(&self) -> Vec<GlyimDiagnostic> {
        let mut diags = Vec::new();
        diags.extend(self.lex_diagnostics.iter().cloned());
        diags.extend(self.parse_diagnostics.iter().cloned());
        diags.extend(self.def_map_diagnostics.iter().cloned());
        diags.extend(self.typeck_diagnostics.iter().cloned());
        diags.extend(self.lower_diagnostics.iter().cloned());
        diags.extend(self.borrowck_diagnostics.iter().cloned());
        diags
    }
    pub fn has_errors(&self) -> bool {
        self.all_diagnostics().iter().any(|d| d.is_error())
    }
}
