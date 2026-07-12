use super::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TokenKind {
    Ident(String),
    String(String),
    Int(i64),
    Float(f64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,
    FatArrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    Equal,
    EqEq,
    BangEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Question,
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub(crate) fn lex(path: &str, source: &str) -> Result<Vec<Token>, Vec<Diagnostic>> {
    let mut lexer = Lexer {
        path,
        source,
        offset: 0,
        diagnostics: Vec::new(),
    };
    let mut tokens = Vec::new();
    loop {
        lexer.skip_trivia();
        let start = lexer.offset;
        if start == source.len() {
            tokens.push(Token {
                kind: TokenKind::Eof,
                span: Span::new(start, start),
            });
            break;
        }
        if let Some(token) = lexer.next_token() {
            tokens.push(token);
        }
    }
    if lexer.diagnostics.is_empty() {
        Ok(tokens)
    } else {
        Err(lexer.diagnostics)
    }
}

struct Lexer<'a> {
    path: &'a str,
    source: &'a str,
    offset: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn next_token(&mut self) -> Option<Token> {
        let start = self.offset;
        let ch = self.bump()?;
        let kind = match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '.' => TokenKind::Dot,
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '?' => TokenKind::Question,
            '-' if self.take('>') => TokenKind::Arrow,
            '-' => TokenKind::Minus,
            '=' if self.take('=') => TokenKind::EqEq,
            '=' if self.take('>') => TokenKind::FatArrow,
            '=' => TokenKind::Equal,
            '!' if self.take('=') => TokenKind::BangEq,
            '!' => TokenKind::Bang,
            '<' if self.take('=') => TokenKind::Le,
            '<' => TokenKind::Lt,
            '>' if self.take('=') => TokenKind::Ge,
            '>' => TokenKind::Gt,
            '&' if self.take('&') => TokenKind::AndAnd,
            '|' if self.take('|') => TokenKind::OrOr,
            '"' => return self.string(start),
            value if value.is_ascii_digit() => return self.number(start),
            value if is_ident_start(value) => return Some(self.ident(start)),
            other => {
                self.diagnostics.push(Diagnostic::error(
                    "E0001",
                    self.path,
                    Span::new(start, self.offset),
                    format!("unexpected character `{other}`"),
                ));
                return None;
            }
        };
        Some(Token {
            kind,
            span: Span::new(start, self.offset),
        })
    }

    fn ident(&mut self, start: usize) -> Token {
        while self.peek().is_some_and(is_ident_continue) {
            self.bump();
        }
        Token {
            kind: TokenKind::Ident(self.source[start..self.offset].to_owned()),
            span: Span::new(start, self.offset),
        }
    }

    fn number(&mut self, start: usize) -> Option<Token> {
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }
        let is_float =
            self.peek() == Some('.') && self.peek_second().is_some_and(|ch| ch.is_ascii_digit());
        if is_float {
            self.bump();
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.bump();
            }
        }
        let text = &self.source[start..self.offset];
        let kind = if is_float {
            text.parse::<f64>().ok().map(TokenKind::Float)
        } else {
            text.parse::<i64>().ok().map(TokenKind::Int)
        };
        match kind {
            Some(kind) => Some(Token {
                kind,
                span: Span::new(start, self.offset),
            }),
            None => {
                self.diagnostics.push(Diagnostic::error(
                    "E0002",
                    self.path,
                    Span::new(start, self.offset),
                    "numeric literal is out of range",
                ));
                None
            }
        }
    }

    fn string(&mut self, start: usize) -> Option<Token> {
        let mut value = String::new();
        while let Some(ch) = self.bump() {
            match ch {
                '"' => {
                    return Some(Token {
                        kind: TokenKind::String(value),
                        span: Span::new(start, self.offset),
                    });
                }
                '\\' => match self.bump() {
                    Some('n') => value.push('\n'),
                    Some('r') => value.push('\r'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    Some(other) => {
                        self.diagnostics.push(Diagnostic::error(
                            "E0003",
                            self.path,
                            Span::new(start, self.offset),
                            format!("unsupported escape `\\{other}`"),
                        ));
                    }
                    None => break,
                },
                other => value.push(other),
            }
        }
        self.diagnostics.push(Diagnostic::error(
            "E0004",
            self.path,
            Span::new(start, self.offset),
            "unterminated string literal",
        ));
        None
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if self.remaining().starts_with("//") {
                while self.peek().is_some_and(|ch| ch != '\n') {
                    self.bump();
                }
                continue;
            }
            if self.remaining().starts_with("/*") {
                let start = self.offset;
                self.offset += 2;
                while self.offset < self.source.len() && !self.remaining().starts_with("*/") {
                    self.bump();
                }
                if self.remaining().starts_with("*/") {
                    self.offset += 2;
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        "E0005",
                        self.path,
                        Span::new(start, self.offset),
                        "unterminated block comment",
                    ));
                }
                continue;
            }
            break;
        }
    }

    fn remaining(&self) -> &str {
        &self.source[self.offset..]
    }
    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }
    fn peek_second(&self) -> Option<char> {
        self.remaining().chars().nth(1)
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }
    fn take(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}
fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_v2_declarations_and_operators() {
        let tokens = lex(
            "main.wms",
            "component Pos persistent { x: int } task go() -> int { return 1 >= 0; }",
        )
        .unwrap();
        assert!(tokens.len() > 20);
        assert!(matches!(
            tokens.last().map(|token| &token.kind),
            Some(TokenKind::Eof)
        ));
    }

    #[test]
    fn rejects_unknown_escape() {
        let errors = lex("main.wms", r#"func f() { return "\q"; }"#).unwrap_err();
        assert_eq!(errors[0].code, "E0003");
    }
}
