use super::{BinaryOp, Expr, Result, Stmt, parse_literal_value, unsupported_expression};

pub(super) fn parse_statements_until(
    parser: &mut ExprParser<'_>,
    terminator: Option<u8>,
) -> Result<Vec<Stmt>> {
    let mut statements = Vec::new();
    loop {
        parser.skip_ws_and_comments();
        if parser.eof() {
            if terminator.is_some() {
                return Err(unsupported_expression("unexpected end of block"));
            }
            break;
        }
        if let Some(terminator) = terminator {
            if parser.peek_byte() == Some(terminator) {
                break;
            }
        }
        statements.push(parser.parse_statement()?);
    }
    Ok(statements)
}
pub(super) struct ExprParser<'a> {
    source: &'a str,
    index: usize,
}

impl<'a> ExprParser<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        Self { source, index: 0 }
    }

    pub(super) fn parse_statements(&mut self, terminator: Option<u8>) -> Result<Vec<Stmt>> {
        parse_statements_until(self, terminator)
    }

    fn parse_statement(&mut self) -> Result<Stmt> {
        self.skip_ws_and_comments();
        if self.consume_keyword("return") {
            self.skip_ws_and_comments();
            if self.consume_byte(b';') {
                return Ok(Stmt::Return(None));
            }
            let expr = self.parse_expression()?;
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Return(Some(expr)));
        }
        if self.consume_keyword("if") {
            return self.parse_if_statement();
        }
        if self.consume_keyword("loop") {
            let body = self.parse_block()?;
            return Ok(Stmt::Loop { body });
        }
        if self.consume_keyword("break") {
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Break);
        }
        if self.consume_keyword("continue") {
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Continue);
        }
        if self.consume_keyword("let") {
            self.skip_ws_and_comments();
            let name = self.read_identifier_source()?;
            self.skip_ws_and_comments();
            let value = if self.consume_byte(b'=') {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.skip_ws_and_comments();
            self.expect_byte(b';')?;
            return Ok(Stmt::Let { name, value });
        }

        let expr = self.parse_expression()?;
        self.skip_ws_and_comments();
        self.expect_byte(b';')?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt> {
        let condition = self.parse_expression()?;
        self.skip_ws_and_comments();
        let then_branch = self.parse_block()?;
        self.skip_ws_and_comments();
        let else_branch = if self.consume_keyword("else") {
            self.skip_ws_and_comments();
            if self.consume_keyword("if") {
                vec![self.parse_if_statement()?]
            } else {
                self.parse_block()?
            }
        } else {
            Vec::new()
        };
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>> {
        self.skip_ws_and_comments();
        self.expect_byte(b'{')?;
        let statements = self.parse_statements(Some(b'}'))?;
        self.skip_ws_and_comments();
        self.expect_byte(b'}')?;
        Ok(statements)
    }

    fn parse_expression(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr> {
        let mut expr = self.parse_and()?;
        loop {
            self.skip_ws_and_comments();
            if !self.source[self.index..].starts_with("||") {
                break;
            }
            self.index += 2;
            let rhs = self.parse_and()?;
            expr = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut expr = self.parse_equality()?;
        loop {
            self.skip_ws_and_comments();
            if !self.source[self.index..].starts_with("&&") {
                break;
            }
            self.index += 2;
            let rhs = self.parse_equality()?;
            expr = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut expr = self.parse_comparison()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.source[self.index..].starts_with("==") {
                self.index += 2;
                Some(BinaryOp::Eq)
            } else if self.source[self.index..].starts_with("!=") {
                self.index += 2;
                Some(BinaryOp::Ne)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_comparison()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut expr = self.parse_additive()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.source[self.index..].starts_with("<=") {
                self.index += 2;
                Some(BinaryOp::Le)
            } else if self.source[self.index..].starts_with(">=") {
                self.index += 2;
                Some(BinaryOp::Ge)
            } else if self.consume_byte(b'<') {
                Some(BinaryOp::Lt)
            } else if self.consume_byte(b'>') {
                Some(BinaryOp::Gt)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_additive()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.consume_byte(b'+') {
                Some(BinaryOp::Add)
            } else if self.consume_byte(b'-') {
                Some(BinaryOp::Sub)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_multiplicative()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut expr = self.parse_unary()?;
        loop {
            self.skip_ws_and_comments();
            let op = if self.consume_byte(b'*') {
                Some(BinaryOp::Mul)
            } else if self.consume_byte(b'/') {
                Some(BinaryOp::Div)
            } else {
                None
            };
            let Some(op) = op else {
                break;
            };
            let rhs = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        self.skip_ws_and_comments();
        if self.consume_byte(b'-') {
            return Ok(Expr::UnaryNeg(Box::new(self.parse_unary()?)));
        }
        if self.consume_byte(b'!') {
            return Ok(Expr::UnaryNot(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        self.skip_ws_and_comments();
        match self.peek_byte() {
            Some(b'(') => {
                self.index += 1;
                let expr = self.parse_expression()?;
                self.skip_ws_and_comments();
                self.expect_byte(b')')?;
                Ok(expr)
            }
            Some(b'"') => {
                let literal = self.read_string_literal_source()?;
                Ok(Expr::Literal(parse_literal_value(&literal)?))
            }
            Some(byte) if is_literal_start(byte) => {
                let ident = self.read_identifier_source()?;
                let mut path = vec![ident];
                while self.consume_byte(b'.') {
                    path.push(self.read_identifier_source()?);
                }
                self.skip_ws_and_comments();
                if self.consume_byte(b'(') {
                    let mut args = Vec::new();
                    self.skip_ws_and_comments();
                    if !self.consume_byte(b')') {
                        loop {
                            args.push(self.parse_expression()?);
                            self.skip_ws_and_comments();
                            if self.consume_byte(b')') {
                                break;
                            }
                            self.expect_byte(b',')?;
                        }
                    }
                    Ok(Expr::Call { path, args })
                } else if path.len() == 1 {
                    if let Ok(value) = parse_literal_value(&path[0]) {
                        Ok(Expr::Literal(value))
                    } else {
                        Ok(Expr::Variable(path[0].clone()))
                    }
                } else {
                    Err(unsupported_expression(format!(
                        "unexpected path expression `{}`",
                        path.join(".")
                    )))
                }
            }
            Some(_) => Err(unsupported_expression("unexpected token in expression")),
            None => Err(unsupported_expression("unexpected end of expression")),
        }
    }

    fn read_identifier_source(&mut self) -> Result<String> {
        let start = self.index;
        while let Some(byte) = self.peek_byte() {
            if byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'+' | b'-'
                        | b'*'
                        | b'/'
                        | b'('
                        | b')'
                        | b';'
                        | b','
                        | b'.'
                        | b'='
                        | b'<'
                        | b'>'
                        | b'!'
                        | b'&'
                        | b'|'
                )
            {
                break;
            }
            self.index += 1;
        }
        if start == self.index {
            return Err(unsupported_expression("expected literal"));
        }
        Ok(self.source[start..self.index].to_owned())
    }

    fn read_string_literal_source(&mut self) -> Result<String> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let quote = bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| unsupported_expression("unexpected end of string literal"))?;
        self.index += 1;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            self.index += 1;
            if escaped {
                escaped = false;
                continue;
            }
            if byte == b'\\' {
                escaped = true;
                continue;
            }
            if byte == quote {
                return Ok(self.source[start..self.index].to_owned());
            }
        }
        Err(unsupported_expression("unterminated string literal"))
    }

    fn skip_ws_and_comments(&mut self) {
        let bytes = self.source.as_bytes();
        while let Some(&byte) = bytes.get(self.index) {
            if byte.is_ascii_whitespace() {
                self.index += 1;
                continue;
            }
            if byte == b'/' && bytes.get(self.index + 1) == Some(&b'/') {
                self.index += 2;
                while let Some(&next) = bytes.get(self.index) {
                    self.index += 1;
                    if next == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let end = self.index.saturating_add(keyword.len());
        if self.source[self.index..].starts_with(keyword)
            && self
                .source
                .as_bytes()
                .get(end)
                .map_or(true, |byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        {
            self.index = end;
            true
        } else {
            false
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.peek_byte() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(&mut self, byte: u8) -> Result<()> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            Err(unsupported_expression(format!(
                "expected `{}`",
                byte as char
            )))
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.index).copied()
    }

    fn eof(&self) -> bool {
        self.index >= self.source.len()
    }
}

fn is_literal_start(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
