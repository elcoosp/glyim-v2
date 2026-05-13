use glyim_core::DefId;
use glyim_mir::*;
use glyim_type::TyCtx;
use std::collections::HashMap;

mod interp_error;
mod interp_value;

pub use interp_error::InterpError;
pub use interp_value::InterpValue;

pub struct Interpreter {
    tcx: TyCtx,
    step_limit: usize,
    recursion_limit: usize,
    step_count: usize,
    recursion_depth: usize,
    function_table: HashMap<DefId, Body>,
    current_frame: Option<Frame>,
}

struct Frame {
    locals: Vec<Option<InterpValue>>,
}

impl Interpreter {
    pub fn new(tcx: TyCtx) -> Self {
        Interpreter {
            tcx,
            step_limit: 1_000_000,
            recursion_limit: 256,
            step_count: 0,
            recursion_depth: 0,
            function_table: HashMap::new(),
            current_frame: None,
        }
    }

    pub fn with_step_limit(mut self, limit: usize) -> Self {
        self.step_limit = limit;
        self
    }

    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    pub fn add_function(&mut self, def_id: DefId, body: Body) {
        self.function_table.insert(def_id, body);
    }

    pub fn step_limit(&self) -> usize {
        self.step_limit
    }

    pub fn recursion_limit(&self) -> usize {
        self.recursion_limit
    }

    pub fn run_body(&mut self, _body: &Body) -> InterpResult<()> {
        Err(InterpError::TimedOut)
    }

    pub fn get_local_value(&self, _local: LocalIdx) -> Option<&InterpValue> {
        self.current_frame.as_ref()?.locals.get(_local.index())?.as_ref()
    }
}

pub type InterpResult<T> = Result<T, InterpError>;
