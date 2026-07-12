use std::collections::BTreeMap;
use std::rc::Rc;

use wmbytecode::{Op, encode_op};
use wmvm::{Function, Program, Value};

use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CompileOutput {
    pub program: Program,
    pub entry_points: BTreeMap<HandlerKindKey, u16>,
    pub test_functions: BTreeMap<String, u16>,
    pub schema: Vec<SchemaType>,
    pub systems: Vec<SystemEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemEntry {
    pub name: String,
    pub function_id: u16,
    pub event_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaType {
    pub name: String,
    pub kind: RecordKind,
    pub persistent: bool,
    pub fields: Vec<(u16, String, TypeRef)>,
}

pub(crate) fn lower(checked: &CheckedModule) -> Result<CompileOutput, Vec<Diagnostic>> {
    let mut function_ids = BTreeMap::new();
    let mut next_id = 1u16;
    for name in checked.callables.keys() {
        function_ids.insert(name.clone(), next_id);
        next_id = next_id.saturating_add(1);
    }
    let mut entry_points = BTreeMap::new();
    for kind in checked.handlers.keys() {
        entry_points.insert(*kind, next_id);
        next_id = next_id.saturating_add(1);
    }

    let schema = build_schema(checked)?;
    let fields = schema
        .iter()
        .flat_map(|schema| {
            schema.fields.iter().map(|(_, name, _)| {
                (
                    format!("{}.{}", schema.name, name),
                    field_id(&schema.name, name),
                )
            })
        })
        .collect();
    let mut program = Program::new();
    let mut diagnostics = Vec::new();
    let mut test_functions = BTreeMap::new();
    let mut systems = Vec::new();

    for (name, decl) in &checked.callables {
        let id = function_ids[name];
        match FunctionLowerer::new(
            &checked.module.path,
            &mut program,
            &function_ids,
            &fields,
            &decl.params,
        )
        .lower(&decl.body)
        {
            Ok((code, local_count)) => {
                program.insert_function(Function::new(
                    id,
                    code,
                    decl.params.len() as u8,
                    local_count,
                ));
                if decl.kind == CallableKind::Test {
                    test_functions.insert(name.clone(), id);
                }
                if decl.kind == CallableKind::System {
                    systems.push(SystemEntry {
                        name: name.clone(),
                        function_id: id,
                        event_type: decl.params.first().and_then(|param| match &param.ty {
                            TypeRef::Named(name) => Some(name.clone()),
                            _ => None,
                        }),
                    });
                }
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    for (kind, decl) in &checked.handlers {
        let id = entry_points[kind];
        match FunctionLowerer::new(
            &checked.module.path,
            &mut program,
            &function_ids,
            &fields,
            &[],
        )
        .lower(&decl.body)
        {
            Ok((code, local_count)) => {
                program.insert_function(Function::new(id, code, 0, local_count));
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if let Some(start) = entry_points
        .get(&HandlerKindKey::Start)
        .copied()
        .or_else(|| {
            checked
                .callables
                .keys()
                .next()
                .and_then(|name| function_ids.get(name).copied())
        })
    {
        program.set_entry(start);
    }
    systems.sort_by(|left, right| left.name.cmp(&right.name));
    if diagnostics.is_empty() {
        Ok(CompileOutput {
            program,
            entry_points,
            test_functions,
            schema,
            systems,
        })
    } else {
        Err(diagnostics)
    }
}

fn build_schema(checked: &CheckedModule) -> Result<Vec<SchemaType>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut used = BTreeMap::<u16, String>::new();
    let mut result = Vec::new();
    for record in checked.records.values() {
        let mut fields = Vec::new();
        for field in &record.fields {
            let id = field_id(&record.name, &field.name);
            let full = format!("{}.{}", record.name, field.name);
            if let Some(previous) = used.insert(id, full.clone()) {
                diagnostics.push(Diagnostic::error(
                    "E0300",
                    &checked.module.path,
                    field.span,
                    format!("field id collision between `{previous}` and `{full}`"),
                ));
            }
            if id == 0 {
                diagnostics.push(Diagnostic::error(
                    "E0300",
                    &checked.module.path,
                    field.span,
                    format!("field `{full}` collides with reserved runtime type tag"),
                ));
            }
            fields.push((id, field.name.clone(), field.ty.clone()));
        }
        fields.sort_by_key(|field| field.0);
        result.push(SchemaType {
            name: record.name.clone(),
            kind: record.kind,
            persistent: record.persistent,
            fields,
        });
    }
    if diagnostics.is_empty() {
        Ok(result)
    } else {
        Err(diagnostics)
    }
}

fn field_id(record: &str, field: &str) -> u16 {
    let mut hash = 0x811c9dc5u32;
    for byte in record.bytes().chain([b'.']).chain(field.bytes()) {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    ((hash >> 16) as u16) ^ (hash as u16)
}

struct FunctionLowerer<'a> {
    path: &'a str,
    program: &'a mut Program,
    functions: &'a BTreeMap<String, u16>,
    fields: &'a BTreeMap<String, u16>,
    locals: BTreeMap<String, u8>,
    local_types: BTreeMap<String, TypeRef>,
    next_local: u8,
    loop_stack: Vec<LoopPatch>,
    diagnostics: Vec<Diagnostic>,
}

struct LoopPatch {
    start: u32,
    breaks: Vec<usize>,
}

impl<'a> FunctionLowerer<'a> {
    fn new(
        path: &'a str,
        program: &'a mut Program,
        functions: &'a BTreeMap<String, u16>,
        fields: &'a BTreeMap<String, u16>,
        params: &[ParamDecl],
    ) -> Self {
        let locals = params
            .iter()
            .enumerate()
            .map(|(index, param)| (param.name.clone(), index as u8))
            .collect();
        let local_types = params
            .iter()
            .map(|param| (param.name.clone(), param.ty.clone()))
            .collect();
        Self {
            path,
            program,
            functions,
            fields,
            locals,
            local_types,
            next_local: params.len() as u8,
            loop_stack: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lower(mut self, block: &Block) -> Result<(Vec<u8>, u8), Vec<Diagnostic>> {
        let mut code = Vec::new();
        self.block(block, &mut code);
        if !matches!(code.last(), Some(byte) if *byte == wmbytecode::Opcode::Return as u8) {
            encode_op(&Op::Return, &mut code);
        }
        if self.diagnostics.is_empty() {
            Ok((code, self.next_local))
        } else {
            Err(self.diagnostics)
        }
    }

    fn block(&mut self, block: &Block, code: &mut Vec<u8>) {
        for stmt in &block.statements {
            self.stmt(stmt, code);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, code: &mut Vec<u8>) {
        match stmt {
            Stmt::Let {
                name,
                ty,
                value,
                span,
            } => {
                self.expr(value, code);
                if self.next_local == u8::MAX {
                    self.error(*span, "E0301", "too many local variables");
                    return;
                }
                let local =
                    self.allocate_local(name, ty.clone().or_else(|| expr_declared_type(value)));
                encode_op(&Op::StoreLocal(local), code);
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                if let Expr::Name(name, _) = target {
                    self.expr(value, code);
                    if let Some(local) = self.locals.get(name).copied() {
                        encode_op(&Op::StoreLocal(local), code);
                    } else {
                        self.error(
                            *span,
                            "E0302",
                            format!("cannot assign unknown local `{name}`"),
                        );
                    }
                } else if let Expr::Field { object, name, .. } = target {
                    if let Expr::Name(local_name, _) = object.as_ref() {
                        if let Some(local) = self.locals.get(local_name).copied() {
                            if let Some(field) = self.local_field_id(local_name, name) {
                                encode_op(&Op::LoadLocal(local), code);
                                let field_const =
                                    self.program.push_constant(Value::Integer(i64::from(field)));
                                encode_op(&Op::PushConst(field_const), code);
                                self.expr(value, code);
                                encode_op(
                                    &Op::CallHost(super::standard::host_id::CORE_SET_FIELD, 3),
                                    code,
                                );
                                encode_op(&Op::StoreLocal(local), code);
                            } else {
                                self.error(
                                    *span,
                                    "E0303",
                                    format!("cannot resolve field `{name}` for `{local_name}`"),
                                );
                            }
                        } else {
                            self.error(
                                *span,
                                "E0302",
                                format!("cannot assign unknown local `{local_name}`"),
                            );
                        }
                    } else {
                        self.error(
                            *span,
                            "E0303",
                            "field assignment requires a local record target",
                        );
                    }
                } else if let Expr::Index { object, index, .. } = target {
                    if let Expr::Name(local_name, _) = object.as_ref() {
                        if let Some(local) = self.locals.get(local_name).copied() {
                            encode_op(&Op::LoadLocal(local), code);
                            self.expr(index, code);
                            self.expr(value, code);
                            encode_op(
                                &Op::CallHost(super::standard::host_id::CORE_SET_INDEX, 3),
                                code,
                            );
                            encode_op(&Op::StoreLocal(local), code);
                        } else {
                            self.error(
                                *span,
                                "E0302",
                                format!("cannot assign unknown local `{local_name}`"),
                            );
                        }
                    } else {
                        self.error(
                            *span,
                            "E0303",
                            "index assignment requires a local collection target",
                        );
                    }
                } else {
                    self.error(*span, "E0303", "assignment target is not mutable");
                }
            }
            Stmt::Expr(expr) => {
                self.expr(expr, code);
                encode_op(&Op::Pop, code);
            }
            Stmt::Return(value, _) => {
                if let Some(value) = value {
                    self.expr(value, code);
                }
                encode_op(&Op::Return, code);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.expr(condition, code);
                let false_patch = emit_jump(Op::JumpIfFalse(0), code);
                self.block(then_block, code);
                if let Some(else_block) = else_block {
                    let end_patch = emit_jump(Op::Jump(0), code);
                    let else_start = code.len() as u32;
                    patch_jump(code, false_patch, else_start);
                    self.block(else_block, code);
                    let end = code.len() as u32;
                    patch_jump(code, end_patch, end);
                } else {
                    let end = code.len() as u32;
                    patch_jump(code, false_patch, end);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let start = code.len() as u32;
                self.expr(condition, code);
                let end_patch = emit_jump(Op::JumpIfFalse(0), code);
                self.loop_stack.push(LoopPatch {
                    start,
                    breaks: Vec::new(),
                });
                self.block(body, code);
                encode_op(&Op::Jump(start), code);
                let end = code.len() as u32;
                patch_jump(code, end_patch, end);
                if let Some(loop_patch) = self.loop_stack.pop() {
                    for patch in loop_patch.breaks {
                        patch_jump(code, patch, end);
                    }
                }
            }
            Stmt::Break(span) => {
                if self.loop_stack.is_empty() {
                    self.error(*span, "E0304", "break is outside a loop");
                } else {
                    let patch = emit_jump(Op::Jump(0), code);
                    self.loop_stack.last_mut().unwrap().breaks.push(patch);
                }
            }
            Stmt::Continue(span) => {
                if let Some(loop_patch) = self.loop_stack.last() {
                    encode_op(&Op::Jump(loop_patch.start), code);
                } else {
                    self.error(*span, "E0305", "continue is outside a loop");
                }
            }
            Stmt::Emit(value, _) => {
                self.expr(value, code);
                encode_op(&Op::CallHost(super::standard::host_id::WORLD_EMIT, 1), code);
                encode_op(&Op::Pop, code);
            }
            Stmt::For {
                name,
                iterable,
                body,
                span: _,
            } => {
                self.expr(iterable, code);
                let iterable_local = self.allocate_local("#for_iter", expr_declared_type(iterable));
                encode_op(&Op::StoreLocal(iterable_local), code);
                let zero = self.program.push_constant(Value::Integer(0));
                encode_op(&Op::PushConst(zero), code);
                let index_local = self.allocate_local("#for_index", Some(TypeRef::Int));
                encode_op(&Op::StoreLocal(index_local), code);
                let item_type = match self.local_types.get("#for_iter") {
                    Some(TypeRef::Array(inner)) => Some((**inner).clone()),
                    _ => None,
                };
                let item_local = self.allocate_local(name, item_type);
                let start = code.len() as u32;
                encode_op(&Op::LoadLocal(index_local), code);
                encode_op(&Op::LoadLocal(iterable_local), code);
                encode_op(&Op::CallHost(super::standard::host_id::CORE_LEN, 1), code);
                encode_op(&Op::Lt, code);
                let end_patch = emit_jump(Op::JumpIfFalse(0), code);
                encode_op(&Op::LoadLocal(iterable_local), code);
                encode_op(&Op::LoadLocal(index_local), code);
                encode_op(&Op::LoadIndex, code);
                encode_op(&Op::StoreLocal(item_local), code);
                self.loop_stack.push(LoopPatch {
                    start,
                    breaks: Vec::new(),
                });
                self.block(body, code);
                encode_op(&Op::LoadLocal(index_local), code);
                let one = self.program.push_constant(Value::Integer(1));
                encode_op(&Op::PushConst(one), code);
                encode_op(&Op::Add, code);
                encode_op(&Op::StoreLocal(index_local), code);
                encode_op(&Op::Jump(start), code);
                let end = code.len() as u32;
                patch_jump(code, end_patch, end);
                if let Some(loop_patch) = self.loop_stack.pop() {
                    for patch in loop_patch.breaks {
                        patch_jump(code, patch, end);
                    }
                }
            }
            Stmt::Match { value, arms, .. } => {
                self.expr(value, code);
                let match_local = self.allocate_local("#match", expr_declared_type(value));
                encode_op(&Op::StoreLocal(match_local), code);
                let mut end_patches = Vec::new();
                for arm in arms {
                    match &arm.pattern {
                        Pattern::Wildcard => {
                            self.block(&arm.body, code);
                            end_patches.push(emit_jump(Op::Jump(0), code));
                        }
                        Pattern::Literal(literal) => {
                            encode_op(&Op::LoadLocal(match_local), code);
                            self.literal(literal, code);
                            encode_op(&Op::Eq, code);
                            let next = emit_jump(Op::JumpIfFalse(0), code);
                            self.block(&arm.body, code);
                            end_patches.push(emit_jump(Op::Jump(0), code));
                            let next_target = code.len() as u32;
                            patch_jump(code, next, next_target);
                        }
                        Pattern::Name(name) => {
                            encode_op(&Op::LoadLocal(match_local), code);
                            self.literal(&Literal::String(name.clone()), code);
                            encode_op(&Op::Eq, code);
                            let next = emit_jump(Op::JumpIfFalse(0), code);
                            self.block(&arm.body, code);
                            end_patches.push(emit_jump(Op::Jump(0), code));
                            let next_target = code.len() as u32;
                            patch_jump(code, next, next_target);
                        }
                    }
                }
                let end = code.len() as u32;
                for patch in end_patches {
                    patch_jump(code, patch, end);
                }
            }
        }
    }

    fn expr(&mut self, expr: &Expr, code: &mut Vec<u8>) {
        match expr {
            Expr::Literal(value, _) => self.literal(value, code),
            Expr::Name(name, span) => {
                if let Some(local) = self.locals.get(name).copied() {
                    encode_op(&Op::LoadLocal(local), code);
                } else {
                    self.error(*span, "E0307", format!("unknown runtime value `{name}`"));
                    encode_op(&Op::PushNil, code);
                }
            }
            Expr::Array(values, span) => match constant_array(values) {
                Some(value) => {
                    let id = self.program.push_constant(value);
                    encode_op(&Op::PushConst(id), code);
                }
                None => {
                    self.error(
                        *span,
                        "E0308",
                        "array literal elements must currently be constant",
                    );
                    encode_op(&Op::PushNil, code);
                }
            },
            Expr::Record { name, fields, span } => match constant_record(name, fields) {
                Some(value) => {
                    let id = self.program.push_constant(value);
                    encode_op(&Op::PushConst(id), code);
                }
                None => {
                    self.error(
                        *span,
                        "E0309",
                        "record literal fields must currently be constant",
                    );
                    encode_op(&Op::PushNil, code);
                }
            },
            Expr::Unary { op, value, .. } => {
                self.expr(value, code);
                encode_op(
                    &match op {
                        UnaryOp::Neg => Op::Neg,
                        UnaryOp::Not => Op::Not,
                    },
                    code,
                );
            }
            Expr::Binary {
                op: BinaryOp::And,
                left,
                right,
                ..
            } => {
                self.expr(left, code);
                encode_op(&Op::Dup, code);
                let patch = emit_jump(Op::JumpIfFalse(0), code);
                encode_op(&Op::Pop, code);
                self.expr(right, code);
                let end = code.len() as u32;
                patch_jump(code, patch, end);
            }
            Expr::Binary {
                op: BinaryOp::Or,
                left,
                right,
                ..
            } => {
                self.expr(left, code);
                encode_op(&Op::Dup, code);
                let patch = emit_jump(Op::JumpIfTrue(0), code);
                encode_op(&Op::Pop, code);
                self.expr(right, code);
                let end = code.len() as u32;
                patch_jump(code, patch, end);
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                self.expr(left, code);
                self.expr(right, code);
                encode_op(&binary_op(*op), code);
            }
            Expr::Field { object, name, span } => {
                self.expr(object, code);
                let id = if let Expr::Name(local, _) = object.as_ref() {
                    self.local_field_id(local, name)
                } else {
                    None
                };
                if let Some(id) = id {
                    encode_op(&Op::LoadField(id), code);
                } else {
                    self.error(*span, "E0310", format!("field `{name}` has no schema id"));
                }
            }
            Expr::Index { object, index, .. } => {
                self.expr(object, code);
                self.expr(index, code);
                encode_op(&Op::LoadIndex, code);
            }
            Expr::Call { callee, args, span } => self.call(callee, args, *span, code),
            Expr::Await { value, span } => self.await_expr(value, *span, code),
        }
    }

    fn call(&mut self, callee: &Expr, args: &[Expr], span: Span, code: &mut Vec<u8>) {
        let path = expr_path(callee);
        for arg in args {
            self.expr(arg, code);
        }
        if let Some(id) = path
            .as_ref()
            .and_then(|path| self.functions.get(path))
            .copied()
        {
            encode_op(&Op::Call(id, args.len() as u8), code);
        } else if let Some(id) = path.as_deref().and_then(super::standard::resolve_host) {
            encode_op(&Op::CallHost(id, args.len() as u8), code);
        } else {
            self.error(
                span,
                "E0311",
                format!(
                    "unknown callable `{}`",
                    path.unwrap_or_else(|| "<expression>".to_owned())
                ),
            );
            encode_op(&Op::PushNil, code);
        }
    }

    fn await_expr(&mut self, value: &Expr, span: Span, code: &mut Vec<u8>) {
        let Expr::Call { callee, args, .. } = value else {
            self.error(span, "E0312", "await requires a task or standard wait call");
            self.expr(value, code);
            return;
        };
        let path = expr_path(callee);
        if matches!(path.as_deref(), Some("input.choice" | "input.text")) {
            self.call(callee, args, span, code);
            encode_op(&Op::Pop, code);
            encode_op(&Op::Recv, code);
        } else if path.as_deref() == Some("time.sleep") {
            self.call(callee, args, span, code);
            encode_op(&Op::Pop, code);
            encode_op(&Op::Sleep, code);
            encode_op(&Op::PushNil, code);
        } else if path.as_deref() == Some("game.next_tick") {
            encode_op(&Op::Yield, code);
            encode_op(&Op::PushNil, code);
        } else {
            self.call(callee, args, span, code);
        }
    }

    fn literal(&mut self, value: &Literal, code: &mut Vec<u8>) {
        match value {
            Literal::Nil => encode_op(&Op::PushNil, code),
            Literal::Bool(true) => encode_op(&Op::PushTrue, code),
            Literal::Bool(false) => encode_op(&Op::PushFalse, code),
            Literal::Int(value) => {
                let id = self.program.push_constant(Value::Integer(*value));
                encode_op(&Op::PushConst(id), code);
            }
            Literal::Float(value) => {
                let id = self.program.push_constant(Value::Float(*value));
                encode_op(&Op::PushConst(id), code);
            }
            Literal::String(value) => {
                let id = self.program.push_constant(Value::String(value.clone()));
                encode_op(&Op::PushConst(id), code);
            }
        }
    }
    fn allocate_local(&mut self, name: &str, ty: Option<TypeRef>) -> u8 {
        let local = self.next_local;
        self.next_local = self.next_local.saturating_add(1);
        self.locals.insert(name.to_owned(), local);
        if let Some(ty) = ty {
            self.local_types.insert(name.to_owned(), ty);
        }
        local
    }
    fn local_field_id(&self, local: &str, field: &str) -> Option<u16> {
        let TypeRef::Named(record) = self.local_types.get(local)? else {
            return None;
        };
        self.fields.get(&format!("{record}.{field}")).copied()
    }
    fn error(&mut self, span: Span, code: &'static str, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(code, self.path, span, message));
    }
}

fn constant_array(values: &[Expr]) -> Option<Value> {
    values
        .iter()
        .map(constant_value)
        .collect::<Option<Vec<_>>>()
        .map(|values| Value::Array(Rc::new(values)))
}
fn constant_record(name: &str, fields: &[(String, Expr)]) -> Option<Value> {
    let mut values = fields
        .iter()
        .map(|(field, value)| Some((field_id(name, field), constant_value(value)?)))
        .collect::<Option<BTreeMap<_, _>>>()?;
    values.insert(0, Value::String(name.to_owned()));
    Some(Value::Table(Rc::new(values)))
}
fn constant_value(expr: &Expr) -> Option<Value> {
    match expr {
        Expr::Literal(value, _) => Some(match value {
            Literal::Nil => Value::Nil,
            Literal::Bool(v) => Value::Bool(*v),
            Literal::Int(v) => Value::Integer(*v),
            Literal::Float(v) => Value::Float(*v),
            Literal::String(v) => Value::String(v.clone()),
        }),
        Expr::Array(values, _) => constant_array(values),
        Expr::Record { name, fields, .. } => constant_record(name, fields),
        _ => None,
    }
}
fn binary_op(op: BinaryOp) -> Op {
    match op {
        BinaryOp::Add => Op::Add,
        BinaryOp::Sub => Op::Sub,
        BinaryOp::Mul => Op::Mul,
        BinaryOp::Div => Op::Div,
        BinaryOp::Mod => Op::Mod,
        BinaryOp::Eq => Op::Eq,
        BinaryOp::Ne => Op::Ne,
        BinaryOp::Lt => Op::Lt,
        BinaryOp::Le => Op::Le,
        BinaryOp::Gt => Op::Gt,
        BinaryOp::Ge => Op::Ge,
        BinaryOp::And | BinaryOp::Or => unreachable!(),
    }
}
fn expr_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name, _) => Some(name.clone()),
        Expr::Field { object, name, .. } => Some(format!("{}.{}", expr_path(object)?, name)),
        _ => None,
    }
}
fn expr_declared_type(expr: &Expr) -> Option<TypeRef> {
    match expr {
        Expr::Record { name, .. } => Some(TypeRef::Named(name.clone())),
        Expr::Array(values, _) => Some(TypeRef::Array(Box::new(
            values
                .first()
                .and_then(expr_declared_type)
                .unwrap_or(TypeRef::Any),
        ))),
        Expr::Literal(Literal::Int(_), _) => Some(TypeRef::Int),
        Expr::Literal(Literal::Float(_), _) => Some(TypeRef::Float),
        Expr::Literal(Literal::String(_), _) => Some(TypeRef::String),
        Expr::Literal(Literal::Bool(_), _) => Some(TypeRef::Bool),
        _ => None,
    }
}
fn emit_jump(op: Op, code: &mut Vec<u8>) -> usize {
    let operand = code.len() + 1;
    encode_op(&op, code);
    operand
}
fn patch_jump(code: &mut [u8], operand: usize, target: u32) {
    code[operand..operand + 4].copy_from_slice(&target.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::super::{check_module, parse_module};
    use super::*;
    use wmbytecode::BytecodeCursor;

    #[test]
    fn lowers_start_handler_without_changing_vm_bytecode() {
        let source = "on start { let value: int = 1 + 2; return; }";
        let checked = check_module(parse_module("main.wms", source).unwrap()).unwrap();
        let output = lower(&checked).unwrap();
        let function = output
            .program
            .function(output.program.entry().unwrap())
            .unwrap();
        let mut cursor = BytecodeCursor::new(&function.code);
        assert!(cursor.read_op().is_ok());
    }
}
