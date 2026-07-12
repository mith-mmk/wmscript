use std::fmt;

/// UTF-8 byte range in a source file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
    pub const fn merge(self, other: Self) -> Self {
        Self::new(self.start, other.end)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceModule {
    pub path: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Import(ImportDecl),
    Record(RecordDecl),
    Enum(EnumDecl),
    Callable(CallableDecl),
    Handler(HandlerDecl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDecl {
    pub path: String,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKind {
    Struct,
    Component,
    Resource,
    Event,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordDecl {
    pub kind: RecordKind,
    pub name: String,
    pub persistent: bool,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub ty: TypeRef,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub payload: Vec<TypeRef>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Func,
    Task,
    System,
    Test,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CallableDecl {
    pub kind: CallableKind,
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub return_type: TypeRef,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParamDecl {
    pub name: String,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandlerKind {
    Start,
    Tick,
    Input,
    Message,
    Save,
    Load,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandlerDecl {
    pub kind: HandlerKind,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum TypeRef {
    Nil,
    Bool,
    Int,
    Float,
    String,
    Handle,
    Any,
    Array(Box<TypeRef>),
    Option(Box<TypeRef>),
    Named(String),
}

impl fmt::Display for TypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => f.write_str("nil"),
            Self::Bool => f.write_str("bool"),
            Self::Int => f.write_str("int"),
            Self::Float => f.write_str("float"),
            Self::String => f.write_str("string"),
            Self::Handle => f.write_str("handle"),
            Self::Any => f.write_str("any"),
            Self::Array(v) => write!(f, "Array<{v}>"),
            Self::Option(v) => write!(f, "Option<{v}>"),
            Self::Named(v) => f.write_str(v),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<TypeRef>,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    Expr(Expr),
    Return(Option<Expr>, Span),
    If {
        condition: Expr,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Block,
        span: Span,
    },
    For {
        name: String,
        iterable: Expr,
        body: Block,
        span: Span,
    },
    Match {
        value: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    Emit(Expr, Span),
}

impl Stmt {
    pub const fn span(&self) -> Span {
        match self {
            Self::Let { span, .. }
            | Self::Assign { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::For { span, .. }
            | Self::Match { span, .. }
            | Self::Return(_, span)
            | Self::Break(span)
            | Self::Continue(span)
            | Self::Emit(_, span) => *span,
            Self::Expr(expr) => expr.span(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Wildcard,
    Name(String),
    Literal(Literal),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Literal(Literal, Span),
    Name(String, Span),
    Array(Vec<Expr>, Span),
    Record {
        name: String,
        fields: Vec<(String, Expr)>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Field {
        object: Box<Expr>,
        name: String,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Await {
        value: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub const fn span(&self) -> Span {
        match self {
            Self::Literal(_, s)
            | Self::Name(_, s)
            | Self::Array(_, s)
            | Self::Record { span: s, .. }
            | Self::Unary { span: s, .. }
            | Self::Binary { span: s, .. }
            | Self::Field { span: s, .. }
            | Self::Index { span: s, .. }
            | Self::Call { span: s, .. }
            | Self::Await { span: s, .. } => *s,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
