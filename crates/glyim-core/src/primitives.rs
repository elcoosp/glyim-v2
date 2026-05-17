#[derive(Clone, Debug)]
pub struct TargetInfo {
    pointer_width: u32,
    pub triple: String,
    pub abi: TargetAbi,
}

impl TargetInfo {
    pub fn aarch64() -> Self {
        Self {
            pointer_width: 64,
            triple: "aarch64-unknown-linux-gnu".to_string(),
            abi: TargetAbi::AArch64AAPCS,
        }
    }

    pub fn x86_64() -> Self {
        Self {
            pointer_width: 64,
            triple: "x86_64-unknown-linux-gnu".to_string(),
            abi: TargetAbi::X86_64SystemV,
        }
    }
    pub fn pointer_width(&self) -> u32 {
        self.pointer_width
    }
    pub fn pointer_size(&self) -> u64 {
        self.pointer_width as u64 / 8
    }
    pub fn pointer_align(&self) -> u64 {
        self.pointer_size()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetAbi {
    X86_64SystemV,
    AArch64AAPCS,
}

impl Default for TargetInfo {
    fn default() -> Self {
        Self {
            pointer_width: 64,
            triple: "x86_64-unknown-linux-gnu".to_string(),
            abi: TargetAbi::X86_64SystemV,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    Isize,
}

impl IntTy {
    pub fn bit_width(self, target: &TargetInfo) -> u32 {
        match self {
            Self::I8 => 8,
            Self::I16 => 16,
            Self::I32 => 32,
            Self::I64 => 64,
            Self::Isize => target.pointer_width(),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Isize => "isize",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UintTy {
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl UintTy {
    pub fn bit_width(self, target: &TargetInfo) -> u32 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::U32 => 32,
            Self::U64 => 64,
            Self::Usize => target.pointer_width(),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FloatTy {
    F32,
    F64,
}

impl FloatTy {
    pub fn bit_width(self) -> u32 {
        match self {
            Self::F32 => 32,
            Self::F64 => 64,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mutability {
    Not,
    Mut,
}

impl Mutability {
    pub fn is_mut(self) -> bool {
        matches!(self, Self::Mut)
    }
    pub fn prefix_str(self) -> &'static str {
        if self.is_mut() { "mut " } else { "" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Safety {
    Safe,
    Unsafe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Abi {
    C,
    Glyim,
    System,
}

impl Abi {
    pub fn name(self) -> &'static str {
        match self {
            Self::C => "C",
            Self::Glyim => "glyim",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Gt | Self::LtEq | Self::GtEq
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
    Neg,
    Deref,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Module(u32),
    Inherited,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StructKind {
    Unit,
    Tuple,
    Record,
}
