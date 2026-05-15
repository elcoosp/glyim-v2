#[derive(Debug, Clone)]
pub enum InterpValue {
    Int(i128),
    Bool(bool),
    Unit,
    Aggregate(Vec<InterpValue>),
    Ref(usize),
}

impl PartialEq for InterpValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (InterpValue::Int(a), InterpValue::Int(b)) => a == b,
            (InterpValue::Bool(a), InterpValue::Bool(b)) => a == b,
            (InterpValue::Unit, InterpValue::Unit) => true,
            (InterpValue::Aggregate(a), InterpValue::Aggregate(b)) => a == b,
            (InterpValue::Ref(a), InterpValue::Ref(b)) => a == b,
            _ => false,
        }
    }
}
