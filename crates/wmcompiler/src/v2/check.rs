use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CheckedModule {
    pub module: SourceModule,
    pub records: BTreeMap<String, RecordDecl>,
    pub enums: BTreeMap<String, EnumDecl>,
    pub callables: BTreeMap<String, CallableDecl>,
    pub handlers: BTreeMap<HandlerKindKey, HandlerDecl>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HandlerKindKey {
    Start,
    Tick,
    Input,
    Message,
    Save,
    Load,
}

impl From<HandlerKind> for HandlerKindKey {
    fn from(value: HandlerKind) -> Self {
        match value {
            HandlerKind::Start => Self::Start,
            HandlerKind::Tick => Self::Tick,
            HandlerKind::Input => Self::Input,
            HandlerKind::Message => Self::Message,
            HandlerKind::Save => Self::Save,
            HandlerKind::Load => Self::Load,
        }
    }
}

pub(crate) fn check(module: SourceModule) -> Result<CheckedModule, Vec<Diagnostic>> {
    let mut checker = Checker {
        path: module.path.clone(),
        names: BTreeSet::new(),
        records: BTreeMap::new(),
        enums: BTreeMap::new(),
        callables: BTreeMap::new(),
        handlers: BTreeMap::new(),
        diagnostics: Vec::new(),
    };
    checker.collect(&module);
    checker.validate();
    if checker.diagnostics.is_empty() {
        Ok(CheckedModule {
            module,
            records: checker.records,
            enums: checker.enums,
            callables: checker.callables,
            handlers: checker.handlers,
        })
    } else {
        Err(checker.diagnostics)
    }
}

struct Checker {
    path: String,
    names: BTreeSet<String>,
    records: BTreeMap<String, RecordDecl>,
    enums: BTreeMap<String, EnumDecl>,
    callables: BTreeMap<String, CallableDecl>,
    handlers: BTreeMap<HandlerKindKey, HandlerDecl>,
    diagnostics: Vec<Diagnostic>,
}

impl Checker {
    fn collect(&mut self, module: &SourceModule) {
        for item in &module.items {
            match item {
                Item::Import(_) => {}
                Item::Record(decl) => {
                    if self.insert_name(&decl.name, decl.span) {
                        self.records.insert(decl.name.clone(), decl.clone());
                    }
                }
                Item::Enum(decl) => {
                    if self.insert_name(&decl.name, decl.span) {
                        self.enums.insert(decl.name.clone(), decl.clone());
                    }
                }
                Item::Callable(decl) => {
                    if self.insert_name(&decl.name, decl.span) {
                        self.callables.insert(decl.name.clone(), decl.clone());
                    }
                }
                Item::Handler(decl) => {
                    let key = HandlerKindKey::from(decl.kind);
                    if self.handlers.insert(key, decl.clone()).is_some() {
                        self.diagnostics.push(Diagnostic::error(
                            "E0201",
                            &self.path,
                            decl.span,
                            "duplicate event handler",
                        ));
                    }
                }
            }
        }
    }

    fn insert_name(&mut self, name: &str, span: Span) -> bool {
        if self.names.insert(name.to_owned()) {
            true
        } else {
            self.diagnostics.push(Diagnostic::error(
                "E0200",
                &self.path,
                span,
                format!("duplicate declaration `{name}`"),
            ));
            false
        }
    }

    fn validate(&mut self) {
        let known = self.names.clone();
        let records = self.records.values().cloned().collect::<Vec<_>>();
        for record in &records {
            let mut fields = BTreeSet::new();
            for field in &record.fields {
                if !fields.insert(&field.name) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0202",
                        &self.path,
                        field.span,
                        format!("duplicate field `{}`", field.name),
                    ));
                }
                self.validate_type(&field.ty, field.span, &known);
                if record.persistent && !self.is_persistent_type(&field.ty, &mut BTreeSet::new()) {
                    self.diagnostics.push(Diagnostic::error(
                        "E0203",
                        &self.path,
                        field.span,
                        format!("type `{}` cannot be persisted", field.ty),
                    ));
                }
            }
        }
        let callables = self.callables.values().cloned().collect::<Vec<_>>();
        for decl in &callables {
            self.validate_callable(decl, &known);
        }
        let handlers = self.handlers.values().cloned().collect::<Vec<_>>();
        for handler in &handlers {
            let mut locals = BTreeMap::new();
            self.validate_block(&handler.body, true, &TypeRef::Nil, &mut locals);
        }
    }

    fn validate_callable(&mut self, decl: &CallableDecl, known: &BTreeSet<String>) {
        let mut locals = BTreeMap::new();
        for param in &decl.params {
            self.validate_type(&param.ty, param.span, known);
            if locals
                .insert(param.name.clone(), param.ty.clone())
                .is_some()
            {
                self.diagnostics.push(Diagnostic::error(
                    "E0204",
                    &self.path,
                    param.span,
                    format!("duplicate parameter `{}`", param.name),
                ));
            }
        }
        self.validate_type(&decl.return_type, decl.span, known);
        self.validate_block(
            &decl.body,
            decl.kind == CallableKind::Task,
            &decl.return_type,
            &mut locals,
        );
    }

    fn validate_type(&mut self, ty: &TypeRef, span: Span, known: &BTreeSet<String>) {
        match ty {
            TypeRef::Array(inner) | TypeRef::Option(inner) => {
                self.validate_type(inner, span, known)
            }
            TypeRef::Named(name) if !known.contains(name) => self.diagnostics.push(
                Diagnostic::error("E0205", &self.path, span, format!("unknown type `{name}`")),
            ),
            _ => {}
        }
    }

    fn is_persistent_type(&self, ty: &TypeRef, visiting: &mut BTreeSet<String>) -> bool {
        match ty {
            TypeRef::Nil | TypeRef::Bool | TypeRef::Int | TypeRef::Float | TypeRef::String => true,
            TypeRef::Array(inner) | TypeRef::Option(inner) => {
                self.is_persistent_type(inner, visiting)
            }
            TypeRef::Named(name) if self.enums.contains_key(name) => true,
            TypeRef::Named(name) => {
                if !visiting.insert(name.clone()) {
                    return false;
                }
                let result = self.records.get(name).is_some_and(|record| {
                    record.persistent
                        && record
                            .fields
                            .iter()
                            .all(|field| self.is_persistent_type(&field.ty, visiting))
                });
                visiting.remove(name);
                result
            }
            TypeRef::Handle | TypeRef::Any => false,
        }
    }

    fn validate_block(
        &mut self,
        block: &Block,
        await_allowed: bool,
        return_type: &TypeRef,
        locals: &mut BTreeMap<String, TypeRef>,
    ) {
        for stmt in &block.statements {
            match stmt {
                Stmt::Let {
                    name,
                    ty,
                    value,
                    span,
                } => {
                    let inferred = self.infer_expr(value, await_allowed, locals);
                    if let Some(expected) = ty {
                        if !assignable(expected, &inferred) {
                            self.type_error(*span, expected, &inferred);
                        }
                        locals.insert(name.clone(), expected.clone());
                    } else {
                        locals.insert(name.clone(), inferred);
                    }
                }
                Stmt::Assign {
                    target,
                    value,
                    span,
                } => {
                    let expected = self.infer_expr(target, await_allowed, locals);
                    let actual = self.infer_expr(value, await_allowed, locals);
                    if !assignable(&expected, &actual) {
                        self.type_error(*span, &expected, &actual);
                    }
                }
                Stmt::Expr(expr) | Stmt::Emit(expr, _) => {
                    self.infer_expr(expr, await_allowed, locals);
                }
                Stmt::Return(value, span) => {
                    let actual = value.as_ref().map_or(TypeRef::Nil, |expr| {
                        self.infer_expr(expr, await_allowed, locals)
                    });
                    if !assignable(return_type, &actual) {
                        self.type_error(*span, return_type, &actual);
                    }
                }
                Stmt::If {
                    condition,
                    then_block,
                    else_block,
                    ..
                } => {
                    let actual = self.infer_expr(condition, await_allowed, locals);
                    if !assignable(&TypeRef::Bool, &actual) {
                        self.type_error(condition.span(), &TypeRef::Bool, &actual);
                    }
                    self.validate_block(
                        then_block,
                        await_allowed,
                        return_type,
                        &mut locals.clone(),
                    );
                    if let Some(block) = else_block {
                        self.validate_block(block, await_allowed, return_type, &mut locals.clone());
                    }
                }
                Stmt::While {
                    condition, body, ..
                } => {
                    let actual = self.infer_expr(condition, await_allowed, locals);
                    if !assignable(&TypeRef::Bool, &actual) {
                        self.type_error(condition.span(), &TypeRef::Bool, &actual);
                    }
                    self.validate_block(body, await_allowed, return_type, &mut locals.clone());
                }
                Stmt::For {
                    name,
                    iterable,
                    body,
                    span,
                } => {
                    let ty = self.infer_expr(iterable, await_allowed, locals);
                    let TypeRef::Array(inner) = ty else {
                        self.diagnostics.push(Diagnostic::error(
                            "E0206",
                            &self.path,
                            *span,
                            "for expression must be an Array",
                        ));
                        continue;
                    };
                    let mut child = locals.clone();
                    child.insert(name.clone(), *inner);
                    self.validate_block(body, await_allowed, return_type, &mut child);
                }
                Stmt::Match { value, arms, .. } => {
                    self.infer_expr(value, await_allowed, locals);
                    for arm in arms {
                        self.validate_block(
                            &arm.body,
                            await_allowed,
                            return_type,
                            &mut locals.clone(),
                        );
                    }
                }
                Stmt::Break(_) | Stmt::Continue(_) => {}
            }
        }
    }

    fn infer_expr(
        &mut self,
        expr: &Expr,
        await_allowed: bool,
        locals: &BTreeMap<String, TypeRef>,
    ) -> TypeRef {
        match expr {
            Expr::Literal(value, _) => match value {
                Literal::Nil => TypeRef::Nil,
                Literal::Bool(_) => TypeRef::Bool,
                Literal::Int(_) => TypeRef::Int,
                Literal::Float(_) => TypeRef::Float,
                Literal::String(_) => TypeRef::String,
            },
            Expr::Name(name, span) => locals
                .get(name)
                .cloned()
                .or_else(|| self.callables.get(name).map(|_| TypeRef::Any))
                .unwrap_or_else(|| {
                    if !is_standard_root(name) {
                        self.diagnostics.push(Diagnostic::error(
                            "E0207",
                            &self.path,
                            *span,
                            format!("unknown symbol `{name}`"),
                        ));
                    }
                    TypeRef::Any
                }),
            Expr::Array(values, span) => {
                let mut ty = TypeRef::Any;
                for value in values {
                    let next = self.infer_expr(value, await_allowed, locals);
                    if ty == TypeRef::Any {
                        ty = next;
                    } else if !assignable(&ty, &next) {
                        self.type_error(*span, &ty, &next);
                    }
                }
                TypeRef::Array(Box::new(ty))
            }
            Expr::Record { name, fields, span } => {
                let Some(record) = self.records.get(name).cloned() else {
                    self.diagnostics.push(Diagnostic::error(
                        "E0208",
                        &self.path,
                        *span,
                        format!("unknown record `{name}`"),
                    ));
                    return TypeRef::Any;
                };
                for (field_name, value) in fields {
                    if let Some(field) =
                        record.fields.iter().find(|field| field.name == *field_name)
                    {
                        let actual = self.infer_expr(value, await_allowed, locals);
                        if !assignable(&field.ty, &actual) {
                            self.type_error(value.span(), &field.ty, &actual);
                        }
                    } else {
                        self.diagnostics.push(Diagnostic::error(
                            "E0209",
                            &self.path,
                            value.span(),
                            format!("unknown field `{field_name}` on `{name}`"),
                        ));
                    }
                }
                TypeRef::Named(name.clone())
            }
            Expr::Unary { op, value, span } => {
                let actual = self.infer_expr(value, await_allowed, locals);
                let expected = if *op == UnaryOp::Not {
                    TypeRef::Bool
                } else {
                    actual.clone()
                };
                if (*op == UnaryOp::Not && !assignable(&TypeRef::Bool, &actual))
                    || (*op == UnaryOp::Neg && !is_number(&actual))
                {
                    self.type_error(*span, &expected, &actual);
                }
                expected
            }
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let lhs = self.infer_expr(left, await_allowed, locals);
                let rhs = self.infer_expr(right, await_allowed, locals);
                match op {
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge => TypeRef::Bool,
                    BinaryOp::And | BinaryOp::Or => {
                        if lhs != TypeRef::Bool || rhs != TypeRef::Bool {
                            self.type_error(*span, &TypeRef::Bool, &rhs);
                        }
                        TypeRef::Bool
                    }
                    _ => {
                        if !is_number(&lhs) || !is_number(&rhs) {
                            self.diagnostics.push(Diagnostic::error(
                                "E0210",
                                &self.path,
                                *span,
                                "arithmetic operands must be numeric",
                            ));
                            TypeRef::Any
                        } else if lhs == TypeRef::Float || rhs == TypeRef::Float {
                            TypeRef::Float
                        } else {
                            TypeRef::Int
                        }
                    }
                }
            }
            Expr::Field { object, name, span } => {
                if let Expr::Name(root, _) = object.as_ref() {
                    if root == "ext" || root == "state" {
                        self.diagnostics.push(Diagnostic::error(
                            "E0211",
                            &self.path,
                            *span,
                            format!("legacy namespace `{root}.*` is not available in v2 source"),
                        ));
                    }
                    if is_standard_root(root) {
                        return standard_return_type(root, name);
                    }
                }
                let object_ty = self.infer_expr(object, await_allowed, locals);
                if let TypeRef::Named(record) = object_ty {
                    return self
                        .records
                        .get(&record)
                        .and_then(|decl| decl.fields.iter().find(|field| field.name == *name))
                        .map_or(TypeRef::Any, |field| field.ty.clone());
                }
                TypeRef::Any
            }
            Expr::Index {
                object,
                index,
                span,
            } => {
                let object_ty = self.infer_expr(object, await_allowed, locals);
                let index_ty = self.infer_expr(index, await_allowed, locals);
                if index_ty != TypeRef::Int {
                    self.type_error(*span, &TypeRef::Int, &index_ty);
                }
                if let TypeRef::Array(inner) = object_ty {
                    *inner
                } else {
                    TypeRef::Any
                }
            }
            Expr::Call { callee, args, span } => {
                for arg in args {
                    self.infer_expr(arg, await_allowed, locals);
                }
                if let Expr::Name(name, _) = callee.as_ref()
                    && let Some(callable) = self.callables.get(name)
                {
                    return callable.return_type.clone();
                }
                if let Some(path) = expression_path(callee)
                    && let Some(callable) = self.callables.get(&path)
                {
                    return callable.return_type.clone();
                }
                if let Some((root, name)) = standard_call_path(callee) {
                    return standard_return_type(root, name);
                }
                let callee_ty = self.infer_expr(callee, await_allowed, locals);
                if callee_ty != TypeRef::Any {
                    self.diagnostics.push(Diagnostic::error(
                        "E0212",
                        &self.path,
                        *span,
                        "expression is not callable",
                    ));
                }
                TypeRef::Any
            }
            Expr::Await { value, span } => {
                if !await_allowed {
                    self.diagnostics.push(Diagnostic::error(
                        "E0213",
                        &self.path,
                        *span,
                        "await is only allowed in task and event handlers",
                    ));
                }
                self.infer_expr(value, await_allowed, locals)
            }
        }
    }

    fn type_error(&mut self, span: Span, expected: &TypeRef, actual: &TypeRef) {
        self.diagnostics.push(Diagnostic::error(
            "E0214",
            &self.path,
            span,
            format!("expected `{expected}`, found `{actual}`"),
        ));
    }
}

fn assignable(expected: &TypeRef, actual: &TypeRef) -> bool {
    expected == actual
        || *expected == TypeRef::Any
        || (*expected == TypeRef::Float && *actual == TypeRef::Int)
        || matches!(expected, TypeRef::Option(_)) && *actual == TypeRef::Nil
}
fn is_number(ty: &TypeRef) -> bool {
    matches!(ty, TypeRef::Int | TypeRef::Float | TypeRef::Any)
}
fn is_standard_root(name: &str) -> bool {
    matches!(
        name,
        "core"
            | "game"
            | "world"
            | "time"
            | "random"
            | "input"
            | "scene"
            | "ui"
            | "audio"
            | "asset"
            | "save"
    )
}
fn standard_return_type(root: &str, name: &str) -> TypeRef {
    match (root, name) {
        ("core", "len") => TypeRef::Int,
        ("input", "choice") | ("input", "text") => TypeRef::String,
        ("random", "int") | ("time", "tick") | ("world", "spawn") => TypeRef::Int,
        ("save", "load") => TypeRef::Bool,
        _ => TypeRef::Any,
    }
}
fn standard_call_path(expr: &Expr) -> Option<(&str, &str)> {
    let Expr::Field { object, name, .. } = expr else {
        return None;
    };
    let Expr::Name(root, _) = object.as_ref() else {
        return None;
    };
    is_standard_root(root).then_some((root.as_str(), name.as_str()))
}
fn expression_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Field { object, name, .. } => Some(format!("{}.{}", expression_path(object)?, name)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_module;
    use super::*;

    #[test]
    fn rejects_await_in_system() {
        let module = parse_module("main.wms", "system update() { await time.sleep(1); }").unwrap();
        let errors = check(module).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "E0213"));
    }

    #[test]
    fn rejects_any_in_persistent_schema() {
        let module = parse_module("main.wms", "resource State persistent { value: any }").unwrap();
        let errors = check(module).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "E0203"));
    }
}
