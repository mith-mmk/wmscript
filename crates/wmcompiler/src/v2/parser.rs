use super::ast::*;
use super::diagnostic::Diagnostic;
use super::lexer::{Token, TokenKind, lex};

pub(crate) fn parse(path: &str, source: &str) -> Result<SourceModule, Vec<Diagnostic>> {
    let tokens = lex(path, source)?;
    let mut parser = Parser {
        path,
        tokens,
        cursor: 0,
        diagnostics: Vec::new(),
    };
    let mut items = Vec::new();
    while !parser.at_eof() {
        match parser.item() {
            Ok(item) => items.push(item),
            Err(error) => {
                parser.diagnostics.push(error);
                parser.synchronize_item();
            }
        }
    }
    if parser.diagnostics.is_empty() {
        Ok(SourceModule {
            path: path.to_owned(),
            items,
        })
    } else {
        Err(parser.diagnostics)
    }
}

struct Parser<'a> {
    path: &'a str,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn item(&mut self) -> Result<Item, Diagnostic> {
        if self.take_kw("import").is_some() {
            return self.import().map(Item::Import);
        }
        let start = self.peek().span;
        let kind = if self.take_kw("struct").is_some() {
            Some(RecordKind::Struct)
        } else if self.take_kw("component").is_some() {
            Some(RecordKind::Component)
        } else if self.take_kw("resource").is_some() {
            Some(RecordKind::Resource)
        } else if self.take_kw("event").is_some() {
            Some(RecordKind::Event)
        } else {
            None
        };
        if let Some(kind) = kind {
            return self.record(start, kind).map(Item::Record);
        }
        if self.take_kw("enum").is_some() {
            return self.enum_decl(start).map(Item::Enum);
        }
        if self.take_kw("test").is_some() {
            self.expect_kw("func")?;
            return self.callable(start, CallableKind::Test).map(Item::Callable);
        }
        if self.take_kw("func").is_some() {
            return self.callable(start, CallableKind::Func).map(Item::Callable);
        }
        if self.take_kw("task").is_some() {
            return self.callable(start, CallableKind::Task).map(Item::Callable);
        }
        if self.take_kw("system").is_some() {
            return self
                .callable(start, CallableKind::System)
                .map(Item::Callable);
        }
        if self.take_kw("on").is_some() {
            return self.handler(start).map(Item::Handler);
        }
        Err(self.error("E0100", self.peek().span, "expected a v2 declaration"))
    }

    fn import(&mut self) -> Result<ImportDecl, Diagnostic> {
        let start = self.previous().span;
        let (path, _) = self.string()?;
        let alias = if self.take_kw("as").is_some() {
            Some(self.ident()?.0)
        } else {
            None
        };
        let end = self.expect(TokenTag::Semicolon)?.span;
        Ok(ImportDecl {
            path,
            alias,
            span: start.merge(end),
        })
    }

    fn record(&mut self, start: Span, kind: RecordKind) -> Result<RecordDecl, Diagnostic> {
        let (name, _) = self.ident()?;
        let persistent = self.take_kw("persistent").is_some();
        self.expect(TokenTag::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(TokenTag::RBrace) && !self.at_eof() {
            let (field_name, field_start) = self.ident()?;
            self.expect(TokenTag::Colon)?;
            let ty = self.type_ref()?;
            let default = if self.take(TokenTag::Equal).is_some() {
                Some(self.expr(0)?)
            } else {
                None
            };
            let end = self
                .take(TokenTag::Comma)
                .or_else(|| self.take(TokenTag::Semicolon))
                .map_or_else(
                    || default.as_ref().map_or(field_start, Expr::span),
                    |token| token.span,
                );
            fields.push(FieldDecl {
                name: field_name,
                ty,
                default,
                span: field_start.merge(end),
            });
        }
        let end = self.expect(TokenTag::RBrace)?.span;
        Ok(RecordDecl {
            kind,
            name,
            persistent,
            fields,
            span: start.merge(end),
        })
    }

    fn enum_decl(&mut self, start: Span) -> Result<EnumDecl, Diagnostic> {
        let (name, _) = self.ident()?;
        self.expect(TokenTag::LBrace)?;
        let mut variants = Vec::new();
        while !self.check(TokenTag::RBrace) && !self.at_eof() {
            let (variant_name, variant_start) = self.ident()?;
            let mut payload = Vec::new();
            let mut end = variant_start;
            if self.take(TokenTag::LParen).is_some() {
                if !self.check(TokenTag::RParen) {
                    loop {
                        payload.push(self.type_ref()?);
                        if self.take(TokenTag::Comma).is_none() {
                            break;
                        }
                    }
                }
                end = self.expect(TokenTag::RParen)?.span;
            }
            if let Some(token) = self.take(TokenTag::Comma) {
                end = token.span;
            }
            variants.push(EnumVariant {
                name: variant_name,
                payload,
                span: variant_start.merge(end),
            });
        }
        let end = self.expect(TokenTag::RBrace)?.span;
        Ok(EnumDecl {
            name,
            variants,
            span: start.merge(end),
        })
    }

    fn callable(&mut self, start: Span, kind: CallableKind) -> Result<CallableDecl, Diagnostic> {
        let (name, _) = self.ident()?;
        let params = self.params()?;
        let return_type = if self.take(TokenTag::Arrow).is_some() {
            self.type_ref()?
        } else {
            TypeRef::Nil
        };
        let body = self.block()?;
        let span = start.merge(body.span);
        Ok(CallableDecl {
            kind,
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn params(&mut self) -> Result<Vec<ParamDecl>, Diagnostic> {
        self.expect(TokenTag::LParen)?;
        let mut params = Vec::new();
        if !self.check(TokenTag::RParen) {
            loop {
                let (name, start) = self.ident()?;
                self.expect(TokenTag::Colon)?;
                let ty = self.type_ref()?;
                params.push(ParamDecl {
                    name,
                    ty,
                    span: start,
                });
                if self.take(TokenTag::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect(TokenTag::RParen)?;
        Ok(params)
    }

    fn handler(&mut self, start: Span) -> Result<HandlerDecl, Diagnostic> {
        let (name, span) = self.ident()?;
        let kind = match name.as_str() {
            "start" => HandlerKind::Start,
            "tick" => HandlerKind::Tick,
            "input" => HandlerKind::Input,
            "message" => HandlerKind::Message,
            "save" => HandlerKind::Save,
            "load" => HandlerKind::Load,
            _ => return Err(self.error("E0101", span, format!("unknown handler `{name}`"))),
        };
        let body = self.block()?;
        let end = body.span;
        Ok(HandlerDecl {
            kind,
            body,
            span: start.merge(end),
        })
    }

    fn type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        let (name, span) = self.ident()?;
        let simple = match name.as_str() {
            "nil" => Some(TypeRef::Nil),
            "bool" => Some(TypeRef::Bool),
            "int" => Some(TypeRef::Int),
            "float" => Some(TypeRef::Float),
            "string" => Some(TypeRef::String),
            "handle" => Some(TypeRef::Handle),
            "any" => Some(TypeRef::Any),
            _ => None,
        };
        if let Some(simple) = simple {
            return Ok(simple);
        }
        if name == "Array" || name == "Option" {
            self.expect(TokenTag::Lt)?;
            let inner = self.type_ref()?;
            self.expect(TokenTag::Gt)?;
            return Ok(if name == "Array" {
                TypeRef::Array(Box::new(inner))
            } else {
                TypeRef::Option(Box::new(inner))
            });
        }
        if name.is_empty() {
            Err(self.error("E0102", span, "expected type"))
        } else {
            Ok(TypeRef::Named(name))
        }
    }

    fn block(&mut self) -> Result<Block, Diagnostic> {
        let start = self.expect(TokenTag::LBrace)?.span;
        let mut statements = Vec::new();
        while !self.check(TokenTag::RBrace) && !self.at_eof() {
            statements.push(self.stmt()?);
        }
        let end = self.expect(TokenTag::RBrace)?.span;
        Ok(Block {
            statements,
            span: start.merge(end),
        })
    }

    fn stmt(&mut self) -> Result<Stmt, Diagnostic> {
        if let Some(start) = self.take_kw("let") {
            let (name, _) = self.ident()?;
            let ty = if self.take(TokenTag::Colon).is_some() {
                Some(self.type_ref()?)
            } else {
                None
            };
            self.expect(TokenTag::Equal)?;
            let value = self.expr(0)?;
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Let {
                name,
                ty,
                value,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_kw("return") {
            let value = if self.check(TokenTag::Semicolon) {
                None
            } else {
                Some(self.expr(0)?)
            };
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Return(value, start.span.merge(end)));
        }
        if let Some(start) = self.take_kw("if") {
            let condition = self.expr(0)?;
            let then_block = self.block()?;
            let else_block = if self.take_kw("else").is_some() {
                Some(self.block()?)
            } else {
                None
            };
            let end = else_block
                .as_ref()
                .map_or(then_block.span, |block| block.span);
            return Ok(Stmt::If {
                condition,
                then_block,
                else_block,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_kw("while") {
            let condition = self.expr(0)?;
            let body = self.block()?;
            let end = body.span;
            return Ok(Stmt::While {
                condition,
                body,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_kw("match") {
            let value = self.expr(0)?;
            self.expect(TokenTag::LBrace)?;
            let mut arms = Vec::new();
            while !self.check(TokenTag::RBrace) && !self.at_eof() {
                let pattern_start = self.peek().span;
                let pattern = self.pattern()?;
                self.expect(TokenTag::FatArrow)?;
                let body = self.block()?;
                let end = self
                    .take(TokenTag::Comma)
                    .map_or(body.span, |token| token.span);
                arms.push(MatchArm {
                    pattern,
                    body,
                    span: pattern_start.merge(end),
                });
            }
            let end = self.expect(TokenTag::RBrace)?.span;
            return Ok(Stmt::Match {
                value,
                arms,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_kw("for") {
            let (name, _) = self.ident()?;
            self.expect_kw("in")?;
            let iterable = self.expr(0)?;
            let body = self.block()?;
            let end = body.span;
            return Ok(Stmt::For {
                name,
                iterable,
                body,
                span: start.span.merge(end),
            });
        }
        if let Some(start) = self.take_kw("break") {
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Break(start.span.merge(end)));
        }
        if let Some(start) = self.take_kw("continue") {
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Continue(start.span.merge(end)));
        }
        if let Some(start) = self.take_kw("emit") {
            let value = self.expr(0)?;
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Emit(value, start.span.merge(end)));
        }
        let left = self.expr(0)?;
        if self.take(TokenTag::Equal).is_some() {
            let value = self.expr(0)?;
            let end = self.expect(TokenTag::Semicolon)?.span;
            return Ok(Stmt::Assign {
                span: left.span().merge(end),
                target: left,
                value,
            });
        }
        self.expect(TokenTag::Semicolon)?;
        Ok(Stmt::Expr(left))
    }

    fn expr(&mut self, min_bp: u8) -> Result<Expr, Diagnostic> {
        let mut left = if let Some(token) = self.take_kw("await") {
            let value = self.expr(13)?;
            let span = token.span.merge(value.span());
            Expr::Await {
                value: Box::new(value),
                span,
            }
        } else if let Some(token) = self.take(TokenTag::Minus) {
            let value = self.expr(13)?;
            let span = token.span.merge(value.span());
            Expr::Unary {
                op: UnaryOp::Neg,
                value: Box::new(value),
                span,
            }
        } else if let Some(token) = self.take(TokenTag::Bang) {
            let value = self.expr(13)?;
            let span = token.span.merge(value.span());
            Expr::Unary {
                op: UnaryOp::Not,
                value: Box::new(value),
                span,
            }
        } else {
            self.primary()?
        };

        loop {
            if self.take(TokenTag::LParen).is_some() {
                let mut args = Vec::new();
                if !self.check(TokenTag::RParen) {
                    loop {
                        args.push(self.expr(0)?);
                        if self.take(TokenTag::Comma).is_none() {
                            break;
                        }
                    }
                }
                let end = self.expect(TokenTag::RParen)?.span;
                let span = left.span().merge(end);
                left = Expr::Call {
                    callee: Box::new(left),
                    args,
                    span,
                };
                continue;
            }
            if self.take(TokenTag::Dot).is_some() {
                let (name, end) = self.ident()?;
                let span = left.span().merge(end);
                left = Expr::Field {
                    object: Box::new(left),
                    name,
                    span,
                };
                continue;
            }
            if self.take(TokenTag::LBracket).is_some() {
                let index = self.expr(0)?;
                let end = self.expect(TokenTag::RBracket)?.span;
                let span = left.span().merge(end);
                left = Expr::Index {
                    object: Box::new(left),
                    index: Box::new(index),
                    span,
                };
                continue;
            }
            let Some((left_bp, right_bp, op)) = self.binary_binding() else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            self.cursor += 1;
            let right = self.expr(right_bp)?;
            let span = left.span().merge(right.span());
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Int(v) => {
                self.cursor += 1;
                Ok(Expr::Literal(Literal::Int(v), token.span))
            }
            TokenKind::Float(v) => {
                self.cursor += 1;
                Ok(Expr::Literal(Literal::Float(v), token.span))
            }
            TokenKind::String(v) => {
                self.cursor += 1;
                Ok(Expr::Literal(Literal::String(v), token.span))
            }
            TokenKind::Ident(ref name) if name == "nil" => {
                self.cursor += 1;
                Ok(Expr::Literal(Literal::Nil, token.span))
            }
            TokenKind::Ident(ref name) if name == "true" || name == "false" => {
                let v = name == "true";
                self.cursor += 1;
                Ok(Expr::Literal(Literal::Bool(v), token.span))
            }
            TokenKind::Ident(name) => {
                self.cursor += 1;
                if name.chars().next().is_some_and(char::is_uppercase)
                    && self.take(TokenTag::LBrace).is_some()
                {
                    let mut fields = Vec::new();
                    while !self.check(TokenTag::RBrace) {
                        let (field, _) = self.ident()?;
                        self.expect(TokenTag::Colon)?;
                        fields.push((field, self.expr(0)?));
                        if self.take(TokenTag::Comma).is_none() {
                            break;
                        }
                    }
                    let end = self.expect(TokenTag::RBrace)?.span;
                    Ok(Expr::Record {
                        name,
                        fields,
                        span: token.span.merge(end),
                    })
                } else {
                    Ok(Expr::Name(name, token.span))
                }
            }
            TokenKind::LParen => {
                self.cursor += 1;
                let value = self.expr(0)?;
                self.expect(TokenTag::RParen)?;
                Ok(value)
            }
            TokenKind::LBracket => {
                self.cursor += 1;
                let mut values = Vec::new();
                if !self.check(TokenTag::RBracket) {
                    loop {
                        values.push(self.expr(0)?);
                        if self.take(TokenTag::Comma).is_none() {
                            break;
                        }
                    }
                }
                let end = self.expect(TokenTag::RBracket)?.span;
                Ok(Expr::Array(values, token.span.merge(end)))
            }
            _ => Err(self.error("E0103", token.span, "expected expression")),
        }
    }

    fn pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(name) if name == "_" => {
                self.cursor += 1;
                Ok(Pattern::Wildcard)
            }
            TokenKind::Ident(name) => {
                self.cursor += 1;
                Ok(Pattern::Name(name))
            }
            TokenKind::Int(value) => {
                self.cursor += 1;
                Ok(Pattern::Literal(Literal::Int(value)))
            }
            TokenKind::Float(value) => {
                self.cursor += 1;
                Ok(Pattern::Literal(Literal::Float(value)))
            }
            TokenKind::String(value) => {
                self.cursor += 1;
                Ok(Pattern::Literal(Literal::String(value)))
            }
            _ => Err(self.error("E0108", token.span, "expected match pattern")),
        }
    }

    fn binary_binding(&self) -> Option<(u8, u8, BinaryOp)> {
        Some(match self.peek().kind {
            TokenKind::OrOr => (1, 2, BinaryOp::Or),
            TokenKind::AndAnd => (3, 4, BinaryOp::And),
            TokenKind::EqEq => (5, 6, BinaryOp::Eq),
            TokenKind::BangEq => (5, 6, BinaryOp::Ne),
            TokenKind::Lt => (7, 8, BinaryOp::Lt),
            TokenKind::Le => (7, 8, BinaryOp::Le),
            TokenKind::Gt => (7, 8, BinaryOp::Gt),
            TokenKind::Ge => (7, 8, BinaryOp::Ge),
            TokenKind::Plus => (9, 10, BinaryOp::Add),
            TokenKind::Minus => (9, 10, BinaryOp::Sub),
            TokenKind::Star => (11, 12, BinaryOp::Mul),
            TokenKind::Slash => (11, 12, BinaryOp::Div),
            TokenKind::Percent => (11, 12, BinaryOp::Mod),
            _ => return None,
        })
    }

    fn synchronize_item(&mut self) {
        while !self.at_eof() {
            if self.check_kw("import")
                || self.check_kw("struct")
                || self.check_kw("component")
                || self.check_kw("resource")
                || self.check_kw("event")
                || self.check_kw("enum")
                || self.check_kw("func")
                || self.check_kw("task")
                || self.check_kw("system")
                || self.check_kw("on")
                || self.check_kw("test")
            {
                return;
            }
            self.cursor += 1;
        }
    }

    fn ident(&mut self) -> Result<(String, Span), Diagnostic> {
        let token = self.peek().clone();
        if let TokenKind::Ident(name) = token.kind {
            self.cursor += 1;
            Ok((name, token.span))
        } else {
            Err(self.error("E0104", token.span, "expected identifier"))
        }
    }
    fn string(&mut self) -> Result<(String, Span), Diagnostic> {
        let token = self.peek().clone();
        if let TokenKind::String(value) = token.kind {
            self.cursor += 1;
            Ok((value, token.span))
        } else {
            Err(self.error("E0105", token.span, "expected string literal"))
        }
    }
    fn expect_kw(&mut self, keyword: &str) -> Result<Token, Diagnostic> {
        self.take_kw(keyword)
            .ok_or_else(|| self.error("E0106", self.peek().span, format!("expected `{keyword}`")))
    }
    fn take_kw(&mut self, keyword: &str) -> Option<Token> {
        if self.check_kw(keyword) {
            let token = self.peek().clone();
            self.cursor += 1;
            Some(token)
        } else {
            None
        }
    }
    fn check_kw(&self, keyword: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(name) if name == keyword)
    }
    fn expect(&mut self, tag: TokenTag) -> Result<Token, Diagnostic> {
        self.take(tag).ok_or_else(|| {
            self.error(
                "E0107",
                self.peek().span,
                format!("expected {}", tag.label()),
            )
        })
    }
    fn take(&mut self, tag: TokenTag) -> Option<Token> {
        if self.check(tag) {
            let token = self.peek().clone();
            self.cursor += 1;
            Some(token)
        } else {
            None
        }
    }
    fn check(&self, tag: TokenTag) -> bool {
        tag.matches(&self.peek().kind)
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.cursor.saturating_sub(1)]
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.cursor]
    }
    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }
    fn error(&self, code: &'static str, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, self.path, span, message)
    }
}

#[derive(Clone, Copy)]
enum TokenTag {
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Arrow,
    FatArrow,
    Equal,
    Dot,
    Minus,
    Bang,
    Lt,
    Gt,
}

impl TokenTag {
    fn matches(self, kind: &TokenKind) -> bool {
        matches!(
            (self, kind),
            (Self::LParen, TokenKind::LParen)
                | (Self::RParen, TokenKind::RParen)
                | (Self::LBrace, TokenKind::LBrace)
                | (Self::RBrace, TokenKind::RBrace)
                | (Self::LBracket, TokenKind::LBracket)
                | (Self::RBracket, TokenKind::RBracket)
                | (Self::Comma, TokenKind::Comma)
                | (Self::Colon, TokenKind::Colon)
                | (Self::Semicolon, TokenKind::Semicolon)
                | (Self::Arrow, TokenKind::Arrow)
                | (Self::FatArrow, TokenKind::FatArrow)
                | (Self::Equal, TokenKind::Equal)
                | (Self::Dot, TokenKind::Dot)
                | (Self::Minus, TokenKind::Minus)
                | (Self::Bang, TokenKind::Bang)
                | (Self::Lt, TokenKind::Lt)
                | (Self::Gt, TokenKind::Gt)
        )
    }
    fn label(self) -> &'static str {
        match self {
            Self::LParen => "`(`",
            Self::RParen => "`)`",
            Self::LBrace => "`{`",
            Self::RBrace => "`}`",
            Self::LBracket => "`[`",
            Self::RBracket => "`]`",
            Self::Comma => "`,`",
            Self::Colon => "`:`",
            Self::Semicolon => "`;`",
            Self::Arrow => "`->`",
            Self::FatArrow => "`=>`",
            Self::Equal => "`=`",
            Self::Dot => "`.`",
            Self::Minus => "`-`",
            Self::Bang => "`!`",
            Self::Lt => "`<`",
            Self::Gt => "`>`",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_game_declarations_and_await() {
        let module = parse(
            "main.wms",
            r#"
            component Position persistent { x: int, y: int }
            event Move { entity: int, dx: int }
            task choose() -> string { let route = await input.choice(["a", "b"]); return route; }
            on start { await choose(); }
        "#,
        )
        .unwrap();
        assert_eq!(module.items.len(), 4);
    }

    #[test]
    fn parser_reports_unknown_handler() {
        let errors = parse("main.wms", "on frame { return; }").unwrap_err();
        assert_eq!(errors[0].code, "E0101");
    }
}
