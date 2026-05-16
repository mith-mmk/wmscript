use std::fmt;

/// Result type used by the compiler.
pub type Result<T> = core::result::Result<T, CompileError>;

/// Compiler error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileError {
    SourceTooLarge { len: usize, max: usize },
    Parse(ParseError),
    UnknownModule { path: String },
    DuplicateSymbol { name: String },
    UnsupportedExpression { source: String },
    BytecodeOverflow { what: &'static str, value: u32 },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTooLarge { len, max } => {
                write!(f, "source too large: {len} bytes (max {max})")
            }
            Self::Parse(error) => write!(f, "{error}"),
            Self::UnknownModule { path } => write!(f, "unknown module: {path}"),
            Self::DuplicateSymbol { name } => write!(f, "duplicate symbol: {name}"),
            Self::UnsupportedExpression { source } => {
                write!(f, "unsupported expression: {source}")
            }
            Self::BytecodeOverflow { what, value } => {
                write!(f, "{what} does not fit in target bytecode type: {value}")
            }
        }
    }
}

impl std::error::Error for CompileError {}

impl From<ParseError> for CompileError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

/// Parser error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub kind: ParseErrorKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.path, self.line, self.column, self.kind
        )
    }
}

impl std::error::Error for ParseError {}

/// Fine-grained parser error class.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    UnexpectedEof,
    UnexpectedToken { expected: String, found: String },
    InvalidIdentifier(String),
    InvalidStringLiteral,
    InvalidImportSyntax,
    InvalidFunctionSyntax,
    InvalidLetSyntax,
    UnbalancedBraces,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::UnexpectedToken { expected, found } => {
                write!(f, "unexpected token: expected {expected}, found {found}")
            }
            Self::InvalidIdentifier(value) => write!(f, "invalid identifier: {value}"),
            Self::InvalidStringLiteral => f.write_str("invalid string literal"),
            Self::InvalidImportSyntax => f.write_str("invalid import syntax"),
            Self::InvalidFunctionSyntax => f.write_str("invalid function syntax"),
            Self::InvalidLetSyntax => f.write_str("invalid let syntax"),
            Self::UnbalancedBraces => f.write_str("unbalanced braces"),
        }
    }
}
