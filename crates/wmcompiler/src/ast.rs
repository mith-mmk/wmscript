/// Top-level module AST.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleAst {
    pub path: String,
    pub items: Vec<ModuleItem>,
}

/// AST item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleItem {
    Import(ImportDecl),
    Function(FunctionDecl),
    Let(LetDecl),
}

/// Import declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDecl {
    pub path: String,
    pub alias: Option<String>,
}

/// Function declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub exported: bool,
    pub name: String,
    pub params: Vec<String>,
    pub body: String,
}

/// Top-level global binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LetDecl {
    pub exported: bool,
    pub name: String,
    pub value: String,
}
