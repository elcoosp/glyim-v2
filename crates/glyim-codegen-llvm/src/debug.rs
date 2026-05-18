use glyim_span::FileId;
use inkwell::context::Context;
use inkwell::debug_info::DIScope;
use inkwell::values::FunctionValue;
use std::collections::HashMap;

pub(crate) struct DebugInfoCtx<'ctx> {
    #[allow(dead_code)] // Will be used when debug info is fully implemented
    pub(crate) enabled: bool,
    _source_map: HashMap<FileId, (String, String)>,
    _compile_unit_scope: Option<DIScope<'ctx>>,
}

impl<'ctx> DebugInfoCtx<'ctx> {
    pub(crate) fn new(
        _context: &'ctx Context,
        _module: &inkwell::module::Module<'ctx>,
        source_map: HashMap<FileId, (String, String)>,
        enable: bool,
    ) -> Self {
        if enable {
            tracing::debug!("STUB: debug info generation not fully implemented");
        }
        DebugInfoCtx {
            enabled: enable,
            _source_map: source_map,
            _compile_unit_scope: None,
        }
    }

    #[allow(dead_code)] // Will be used when debug info is fully implemented
    pub(crate) fn set_function(
        &mut self,
        _context: &'ctx Context,
        _func: &FunctionValue<'ctx>,
        _name: &str,
        _file_id: FileId,
        _line: u32,
    ) {
        if self.enabled {
            tracing::trace!("STUB: set_function debug info for {}", _name);
        }
    }

    pub(crate) fn finalize(self) {
        // No-op for now
    }
}
