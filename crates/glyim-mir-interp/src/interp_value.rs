#[derive(Debug, PartialEq, Clone)]
pub enum InterpValue {
    Int(i128),
    Bool(bool),
    Unit,
}
