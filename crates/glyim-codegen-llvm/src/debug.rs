#![allow(dead_code)]
use glyim_mir::VarDebugInfo;
use glyim_span::{FileId, HygieneCtx, Span};
use glyim_type::TyCtx;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DIFile, DIFlagsConstants, DIScope, DISubprogram, DWARFEmissionKind,
    DWARFSourceLanguage, DebugInfoBuilder,
};
use inkwell::values::FunctionValue;
use std::collections::HashMap;

/// Walk back through macro expansions to find the original source location.
pub(crate) fn resolve_span_to_location(mut span: Span, hygiene: &HygieneCtx) -> Span {
    if span.is_dummy() {
        return Span::DUMMY;
    }
    while !span.ctx.is_root() {
        let expn_id = span.ctx.expn_id();
        let expn_data = hygiene.expn_data(expn_id).expect("ExpnData missing");
        span = expn_data.call_site;
    }
    span
}

pub(crate) struct DebugInfoCtx<'ctx> {
    pub(crate) builder: DebugInfoBuilder<'ctx>,
    pub(crate) compile_unit_scope: DIScope<'ctx>,
    pub(crate) subprogram: Option<DISubprogram<'ctx>>,
    files: HashMap<FileId, DIFile<'ctx>>,
    source_map: HashMap<FileId, (String, String)>,
    pub(crate) enabled: bool,
    hygiene: Option<HygieneCtx>,
}

impl<'ctx> DebugInfoCtx<'ctx> {
    pub(crate) fn new(
        _context: &'ctx Context,
        module: &inkwell::module::Module<'ctx>,
        source_map: HashMap<FileId, (String, String)>,
        enable: bool,
        hygiene: Option<HygieneCtx>,
    ) -> Self {
        let (builder, compile_unit) = module.create_debug_info_builder(
            true,
            DWARFSourceLanguage::Rust,
            "test.g",
            ".",
            "glyim",
            false,
            "",
            0u32,
            "",
            DWARFEmissionKind::Full,
            0u32,
            true,
            false,
            "",
            "",
        );
        let compile_unit_scope = compile_unit.as_debug_info_scope();
        let mut files = HashMap::new();
        for (file_id, (path, _source)) in &source_map {
            let dir = std::path::Path::new(path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".");
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(path);
            let file = builder.create_file(filename, dir);
            files.insert(*file_id, file);
        }
        DebugInfoCtx {
            builder,
            compile_unit_scope,
            subprogram: None,
            files,
            source_map,
            enabled: enable,
            hygiene,
        }
    }

    pub(crate) fn set_function(
        &mut self,
        _context: &'ctx Context,
        func: &FunctionValue<'ctx>,
        name: &str,
        file_id: FileId,
        line: u32,
    ) {
        if !self.enabled {
            return;
        }
        let file = self.get_file(file_id);
        let subroutine_type =
            self.builder
                .create_subroutine_type(file, None, &[], DIFlagsConstants::ZERO);
        let subprogram = self.builder.create_function(
            self.compile_unit_scope,
            name,
            Some(name),
            file,
            line,
            subroutine_type,
            false,
            true,
            line,
            DIFlagsConstants::ZERO,
            false,
        );
        func.set_subprogram(subprogram);
        self.subprogram = Some(subprogram);
    }

    pub(crate) fn location_for_span(
        &self,
        context: &'ctx Context,
        span: &Span,
    ) -> Option<inkwell::debug_info::DILocation<'ctx>> {
        if !self.enabled || self.subprogram.is_none() {
            return None;
        }
        let resolved_span = if let Some(ref hygiene) = self.hygiene {
            resolve_span_to_location(*span, hygiene)
        } else {
            *span
        };
        let (line, col) = self.span_to_line_col(&resolved_span)?;
        let scope = self.subprogram.unwrap().as_debug_info_scope();
        Some(
            self.builder
                .create_debug_location(context, line, col, scope, None),
        )
    }

    /// Return a DIType for a given Ty (stub – always i32).
    /// The returned DIType can be used with create_auto_variable.
    pub(crate) fn debug_type_for_ty(
        &self,
        context: &'ctx Context,
        _ty: glyim_type::Ty,
        _ty_ctx: &TyCtx,
    ) -> inkwell::debug_info::DIType<'ctx> {
        self.builder.create_basic_type("i32", 32, 0x05, 0).unwrap().as_type()
    }

    pub(crate) fn declare_local(
        &self,
        context: &'ctx Context,
        alloca: inkwell::values::PointerValue<'ctx>,
        var_info: &VarDebugInfo,
        ty_ctx: &TyCtx,
        block: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        if !self.enabled || self.subprogram.is_none() {
            return;
        }

        let file = self.get_file(FileId::from_raw(0));
        let scope = self.subprogram.unwrap().as_debug_info_scope();

        // Since VarDebugInfo doesn't have a type field, we use a hardcoded i32.
        // TODO: propagate the actual type from the local declaration.
        let basic_ty = self.debug_type_for_ty(context, ty_ctx.error_ty(), ty_ctx);
        let name = ty_ctx.name_str(var_info.name);
        let divar = self.builder.create_auto_variable(
            scope,
            name,
            file,
            1,
            basic_ty,
            true,
            DIFlagsConstants::ZERO,
            32,
        );

        let loc = self
            .builder
            .create_debug_location(context, 1, 1, scope, None);
        let expr = self.builder.create_expression(vec![]);

        self.builder
            .insert_declare_at_end(alloca, Some(divar), Some(expr), loc, block);
    }

    pub(crate) fn finalize(self) {
        if self.enabled {
            self.builder.finalize();
        }
    }

    fn get_file(&self, file_id: FileId) -> DIFile<'ctx> {
        self.files.get(&file_id).copied().unwrap_or_else(|| {
            self.files
                .values()
                .next()
                .copied()
                .unwrap_or_else(|| self.builder.create_file("unknown.g", "."))
        })
    }

    fn span_to_line_col(&self, span: &Span) -> Option<(u32, u32)> {
        if span.is_dummy() {
            return None;
        }
        let file_id = span.file;
        let (_path, source) = self.source_map.get(&file_id)?;
        let offset: usize = span.lo.to_usize();
        if offset >= source.len() {
            return None;
        }
        let prefix = &source[..offset];
        let line = prefix.lines().count() as u32;
        let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = (offset - last_line_start) as u32;
        Some((line, col))
    }
}
