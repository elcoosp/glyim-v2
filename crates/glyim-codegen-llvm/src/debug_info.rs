use glyim_mir::{VarDebugInfo, VarDebugInfoValue};
use glyim_span::{FileId, Span};
use glyim_type::TyCtx;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DIFile, DIFlagsConstants, DIScope, DISubprogram, DWARFEmissionKind,
    DWARFSourceLanguage, DebugInfoBuilder,
};
use inkwell::values::FunctionValue;
use std::collections::HashMap;

pub(crate) struct DebugInfoCtx<'ctx> {
    pub(crate) builder: DebugInfoBuilder<'ctx>,
    pub(crate) compile_unit_scope: DIScope<'ctx>,
    pub(crate) subprogram: Option<DISubprogram<'ctx>>,
    files: HashMap<FileId, DIFile<'ctx>>,
    source_map: HashMap<FileId, (String, String)>,
    pub(crate) enabled: bool,
}

impl<'ctx> DebugInfoCtx<'ctx> {
    pub(crate) fn new(
        _context: &'ctx Context,
        module: &inkwell::module::Module<'ctx>,
        source_map: HashMap<FileId, (String, String)>,
        enable: bool,
    ) -> Self {
        let (builder, compile_unit) = module.create_debug_info_builder(
            true,
            DWARFSourceLanguage::C,
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
        let (line, col) = self.span_to_line_col(span)?;
        let scope = self.subprogram.unwrap().as_debug_info_scope();
        Some(
            self.builder
                .create_debug_location(context, line, col, scope, None),
        )
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
        let place = match &var_info.value {
            VarDebugInfoValue::Place(p) => p,
            _ => return,
        };

        let name = ty_ctx.name_str(var_info.name);
        let file = self.get_file_for_place(place, ty_ctx);
        let (line, col) = self
            .span_for_place(place, ty_ctx)
            .and_then(|s| self.span_to_line_col(&s))
            .unwrap_or((1, 0));

        let di_type = self
            .builder
            .create_basic_type(name, 64, 8, DIFlagsConstants::ZERO)
            .expect("Failed to create DIBasicType")
            .as_type();

        let scope = self.subprogram.unwrap().as_debug_info_scope();

        let local_var = self.builder.create_auto_variable(
            scope,
            name,
            file,
            line,
            di_type,
            false,
            DIFlagsConstants::ZERO,
            0,
        );

        let location = self
            .builder
            .create_debug_location(context, line, col, scope, None);

        let _ = self.builder.insert_declare_at_end(
            alloca,
            Some(local_var),
            None, // expression
            location,
            block,
        );
    }

    pub(crate) fn finalize(self) {
        if self.enabled {
            self.builder.finalize();
        }
    }

    fn get_file(&self, file_id: FileId) -> DIFile<'ctx> {
        self.files.get(&file_id).copied().unwrap_or_else(|| {
            // Return any available file, or create a dummy file if map is empty.
            // In practice, files is always populated when enabled, but we guard for safety.
            self.files
                .values()
                .next()
                .copied()
                .unwrap_or_else(|| self.builder.create_file("unknown.g", "."))
        })
    }

    fn get_file_for_place(&self, _place: &glyim_mir::Place, _ty_ctx: &TyCtx) -> DIFile<'ctx> {
        self.files
            .values()
            .next()
            .copied()
            .unwrap_or_else(|| self.builder.create_file("unknown.g", "."))
    }

    fn span_for_place(&self, _place: &glyim_mir::Place, _ty_ctx: &TyCtx) -> Option<Span> {
        None
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
