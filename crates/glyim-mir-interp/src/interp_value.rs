#[derive(Debug, Clone)]
pub enum InterpValue {
    Int(i128),
    Uint(u128),
    Bool(bool),
    Unit,
    Aggregate(Vec<InterpValue>),
    Ref(usize),
    Float(f64),
    String(String),
    Fn(glyim_core::DefId),
    ConstRef(glyim_core::DefId),
}

impl PartialEq for InterpValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InterpValue::Int(a), InterpValue::Int(b)) => a == b,
            (InterpValue::Uint(a), InterpValue::Uint(b)) => a == b,
            (InterpValue::Bool(a), InterpValue::Bool(b)) => a == b,
            (InterpValue::Unit, InterpValue::Unit) => true,
            (InterpValue::Aggregate(a), InterpValue::Aggregate(b)) => a == b,
            (InterpValue::Ref(a), InterpValue::Ref(b)) => a == b,
            (InterpValue::Float(a), InterpValue::Float(b)) => a.to_bits() == b.to_bits(),
            (InterpValue::String(a), InterpValue::String(b)) => a == b,
            (InterpValue::Fn(a), InterpValue::Fn(b)) => a == b,
            (InterpValue::ConstRef(a), InterpValue::ConstRef(b)) => a == b,
            _ => false,
        }
    }
}
