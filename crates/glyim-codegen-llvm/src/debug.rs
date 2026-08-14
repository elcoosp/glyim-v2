#![allow(dead_code)]
use glyim_core::TargetInfo;
use glyim_layout::{Layout, LayoutComputer, SimpleLayoutComputer};
use glyim_mir::VarDebugInfo;
use glyim_span::{FileId, HygieneCtx, Span};
use glyim_type::TyCtx;
use inkwell::context::Context;
use inkwell::debug_info::{
    AsDIScope, DIFile, DIFlags, DIFlagsConstants, DIScope, DISubprogram, DIType, DWARFEmissionKind,
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
        match hygiene.expn_data(expn_id) {
            Some(expn_data) => span = expn_data.call_site,
            None => break,
        }
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
    type_cache: HashMap<glyim_type::Ty, DIType<'ctx>>,
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
            type_cache: HashMap::new(),
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
        let subroutine_type = self
            .builder
            .create_subroutine_type(file, None, &[], DIFlags::ZERO);
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
            DIFlags::ZERO,
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

    /// Get a debug type for a given Ty, creating it if necessary.
    /// Returns a DIType that can be used directly in debug info.
    pub(crate) fn debug_type_for_ty(
        &mut self,
        context: &'ctx Context,
        ty: glyim_type::Ty,
        ty_ctx: &TyCtx,
    ) -> DIType<'ctx> {
        use glyim_type::{GenericArg, TyKind};
        let target = TargetInfo::default();
        let layout_computer = SimpleLayoutComputer::new(ty_ctx, target);

        if let Some(cached) = self.type_cache.get(&ty) {
            return *cached;
        }

        let file = self.get_file(FileId::from_raw(0));
        let di_type = match ty_ctx.ty_kind(ty) {
            TyKind::Adt(adt_id, _substs) => {
                let adt_def_opt = ty_ctx.adt_def(*adt_id);
                let name = format!("Adt{}", adt_id.to_raw());
                let _ = adt_def_opt;
                let layout = layout_computer
                    .layout_of(ty)
                    .unwrap_or_else(|_| Layout::unit());
                let size_bits = layout.size.0 * 8;
                let align_bits = layout.align.0 * 8;
                if let Some(adt_def) = adt_def_opt {
                    if adt_def.variants.len() == 1 {
                        let variant = &adt_def.variants[0];
                        let mut field_types = Vec::new();
                        for field in variant.fields.iter() {
                            let field_ty = self.debug_type_for_ty(context, field.ty, ty_ctx);
                            field_types.push(field_ty);
                        }
                        self.builder
                            .create_struct_type(
                                self.compile_unit_scope,
                                &name,
                                file,
                                0,
                                size_bits.try_into().unwrap(),
                                align_bits.try_into().unwrap(),
                                0,
                                None,
                                field_types.as_slice(),
                                0,
                                None,
                                "glyim",
                            )
                            .as_type()
                    } else {
                        let tag_bits =
                            if let glyim_layout::VariantsShape::Multiple { tag_size, .. } =
                                &layout.variants
                            {
                                (tag_size.0 * 8).max(8) as u64
                            } else {
                                32
                            };
                        let tag_ty = self
                            .builder
                            .create_basic_type("tag", tag_bits, 0x05, 0)
                            .unwrap()
                            .as_type();
                        let data_size = layout.size.0 * 8 - tag_bits;
                        let data_ty = self
                            .builder
                            .create_basic_type("i8", 8, 0x05, 0)
                            .unwrap()
                            .as_type();
                        let data_array = self.builder.create_array_type(
                            data_ty,
                            (data_size / 8).try_into().unwrap(),
                            0,
                            &[],
                        );
                        self.builder
                            .create_struct_type(
                                self.compile_unit_scope,
                                &format!("{}_enum", name),
                                file,
                                0,
                                size_bits.try_into().unwrap(),
                                align_bits.try_into().unwrap(),
                                0,
                                None,
                                &[tag_ty, data_array.as_type()],
                                0,
                                None,
                                "glyim",
                            )
                            .as_type()
                    }
                } else {
                    self.builder
                        .create_basic_type("<unresolved>", 32, 0x05, 0)
                        .unwrap()
                        .as_type()
                }
            }
            TyKind::Tuple(substs) => {
                let args = ty_ctx.substitution_args(*substs);
                let mut field_types = Vec::new();
                let mut name_parts = Vec::new();
                for (_i, arg) in args.iter().enumerate() {
                    if let GenericArg::Ty(t) = arg {
                        let field_ty = self.debug_type_for_ty(context, *t, ty_ctx);
                        field_types.push(field_ty);
                        name_parts.push(format!("{:?}", t));
                    }
                }
                let name = format!("({})", name_parts.join(", "));
                let layout = layout_computer
                    .layout_of(ty)
                    .unwrap_or_else(|_| Layout::unit());
                let size_bits = layout.size.0 * 8;
                let align_bits = layout.align.0 * 8;
                self.builder
                    .create_struct_type(
                        self.compile_unit_scope,
                        &name,
                        file,
                        0,
                        size_bits.try_into().unwrap(),
                        align_bits.try_into().unwrap(),
                        0,
                        None,
                        field_types.as_slice(),
                        0,
                        None,
                        "glyim",
                    )
                    .as_type()
            }
            TyKind::Array(elem_ty, count) => {
                let count_val = match &count.kind {
                    glyim_type::ConstKind::Uint(n) => *n as u64,
                    glyim_type::ConstKind::Int(n) if *n >= 0 => *n as u64,
                    _ => 0,
                };
                let elem_di = self.debug_type_for_ty(context, *elem_ty, ty_ctx);
                self.builder
                    .create_array_type(elem_di, count_val.try_into().unwrap(), 0, &[])
                    .as_type()
            }
            TyKind::Slice(elem_ty) => {
                let _elem_di = self.debug_type_for_ty(context, *elem_ty, ty_ctx);
                let ptr_ty = self
                    .builder
                    .create_basic_type("ptr", 64, 0x02, 0)
                    .unwrap()
                    .as_type();
                let len_ty = self
                    .builder
                    .create_basic_type("usize", 64, 0x04, 0)
                    .unwrap()
                    .as_type();
                let name = format!("&[{:?}]", elem_ty);
                self.builder
                    .create_struct_type(
                        self.compile_unit_scope,
                        &name,
                        file,
                        0,
                        128, // 2 pointers * 64 bits
                        64,
                        0,
                        None,
                        &[ptr_ty, len_ty],
                        0,
                        None,
                        "glyim",
                    )
                    .as_type()
            }
            _ => self
                .builder
                .create_basic_type("<unresolved>", 32, 0x05, 0)
                .unwrap()
                .as_type(),
        };
        self.type_cache.insert(ty, di_type);
        di_type
    }

    pub(crate) fn declare_local(
        &mut self,
        context: &'ctx Context,
        alloca: inkwell::values::PointerValue<'ctx>,
        var_info: &VarDebugInfo,
        ty: glyim_type::Ty,
        span: glyim_span::Span,
        ty_ctx: &TyCtx,
        block: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        if !self.enabled || self.subprogram.is_none() {
            return;
        }

        let scope = self.subprogram.unwrap().as_debug_info_scope();

        // Use debug type for the variable.
        let basic_ty = self.debug_type_for_ty(context, ty, ty_ctx);
        let name = ty_ctx.name_str(var_info.name);

        // Extract line and column from the variable's declaration span.
        let (line, col) = self.span_to_line_col(&span).unwrap_or((1, 1));
        let file = self.get_file(span.file);

        let divar = self.builder.create_auto_variable(
            scope,
            name,
            file,
            line,
            basic_ty,
            true,
            DIFlags::ZERO,
            32,
        );

        let loc = self
            .builder
            .create_debug_location(context, line, col, scope, None);
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
        let mut offset: usize = span.lo.to_usize();

        if offset > source.len() {
            return None;
        }

        // Walk backward to the nearest UTF-8 character boundary
        while offset > 0 && !source.is_char_boundary(offset) {
            offset -= 1;
        }

        let prefix = &source[..offset];
        let line = prefix.matches('\n').count() as u32 + 1;
        let last_line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = (offset - last_line_start) as u32;
        Some((line, col))
    }
}
