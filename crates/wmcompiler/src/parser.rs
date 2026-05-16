use crate::{FunctionDecl, ImportDecl, LetDecl, ModuleAst, ModuleItem, ParseError, ParseErrorKind};

pub(crate) struct Parser<'a> {
    path: String,
    source: &'a str,
    index: usize,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(path: String, source: &'a str) -> Self {
        Self {
            path,
            source,
            index: 0,
        }
    }

    pub(crate) fn parse_module(mut self) -> std::result::Result<ModuleAst, ParseError> {
        let mut items = Vec::new();
        loop {
            self.skip_ws_and_comments();
            if self.eof() {
                break;
            }
            if self.consume_keyword("import") {
                items.push(ModuleItem::Import(self.parse_import()?));
                continue;
            }
            let exported = self.consume_keyword("export");
            self.skip_ws_and_comments();
            if self.consume_keyword("func") {
                items.push(ModuleItem::Function(self.parse_function(exported)?));
                continue;
            }
            if self.consume_keyword("let") {
                items.push(ModuleItem::Let(self.parse_let(exported)?));
                continue;
            }
            let found = self.peek_token().unwrap_or_else(|| "<eof>".to_owned());
            return Err(self.error(ParseErrorKind::UnexpectedToken {
                expected: "import, export func, or let".to_owned(),
                found,
            }));
        }

        Ok(ModuleAst {
            path: self.path,
            items,
        })
    }

    fn parse_import(&mut self) -> std::result::Result<ImportDecl, ParseError> {
        self.skip_ws_and_comments();
        let path = self.parse_string_literal()?;
        self.skip_ws_and_comments();
        let alias = if self.consume_keyword("as") {
            self.skip_ws_and_comments();
            Some(self.parse_identifier()?)
        } else {
            None
        };
        self.skip_ws_and_comments();
        self.expect_byte(b';', ";")?;
        Ok(ImportDecl { path, alias })
    }

    fn parse_function(&mut self, exported: bool) -> std::result::Result<FunctionDecl, ParseError> {
        self.skip_ws_and_comments();
        let name = self.parse_identifier()?;
        self.skip_ws_and_comments();
        self.expect_byte(b'(', "(")?;
        let mut params = Vec::new();
        self.skip_ws_and_comments();
        if !self.consume_byte(b')') {
            loop {
                self.skip_ws_and_comments();
                params.push(self.parse_identifier()?);
                self.skip_ws_and_comments();
                if self.consume_byte(b')') {
                    break;
                }
                self.expect_byte(b',', ",")?;
            }
        }
        self.skip_ws_and_comments();
        self.expect_byte(b'{', "{")?;
        let body = self.read_block()?;
        Ok(FunctionDecl {
            exported,
            name,
            params,
            body,
        })
    }

    fn parse_let(&mut self, exported: bool) -> std::result::Result<LetDecl, ParseError> {
        self.skip_ws_and_comments();
        let name = self.parse_identifier()?;
        self.skip_ws_and_comments();
        self.expect_byte(b'=', "=")?;
        self.skip_ws_and_comments();
        let value = self.read_until_semicolon()?;
        Ok(LetDecl {
            exported,
            name,
            value,
        })
    }

    fn read_until_semicolon(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut in_string = false;
        let mut quote = 0u8;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    in_string = false;
                }
                self.index += 1;
                continue;
            }
            if byte == b'\'' || byte == b'"' {
                in_string = true;
                quote = byte;
                self.index += 1;
                continue;
            }
            if byte == b';' {
                let end = self.index;
                self.index += 1;
                return Ok(self.source[start..end].trim().to_owned());
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::UnexpectedEof))
    }

    fn read_block(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut depth = 1usize;
        let mut in_string = false;
        let mut quote = 0u8;
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    in_string = false;
                }
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
            match byte {
                b'\'' | b'"' => {
                    in_string = true;
                    quote = byte;
                }
                b'{' => {
                    depth += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = self.index;
                        self.index += 1;
                        return Ok(self.source[start..end].to_owned());
                    }
                }
                _ => {}
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::UnbalancedBraces))
    }

    fn parse_identifier(&mut self) -> std::result::Result<String, ParseError> {
        let start = self.index;
        let bytes = self.source.as_bytes();
        let first = bytes
            .get(self.index)
            .copied()
            .ok_or_else(|| self.error(ParseErrorKind::UnexpectedEof))?;
        if !is_ident_start(first) {
            return Err(self.error(ParseErrorKind::InvalidIdentifier(
                self.peek_token().unwrap_or_default(),
            )));
        }
        self.index += 1;
        while let Some(&byte) = bytes.get(self.index) {
            if !is_ident_continue(byte) {
                break;
            }
            self.index += 1;
        }
        Ok(self.source[start..self.index].to_owned())
    }

    fn parse_string_literal(&mut self) -> std::result::Result<String, ParseError> {
        self.skip_ws_and_comments();
        if !self.consume_byte(b'"') {
            return Err(self.error(ParseErrorKind::InvalidStringLiteral));
        }
        let start = self.index;
        let bytes = self.source.as_bytes();
        let mut escaped = false;
        while let Some(&byte) = bytes.get(self.index) {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                let end = self.index;
                self.index += 1;
                return Ok(self.source[start..end].to_owned());
            }
            self.index += 1;
        }
        Err(self.error(ParseErrorKind::InvalidStringLiteral))
    }

    fn skip_ws_and_comments(&mut self) -> bool {
        let bytes = self.source.as_bytes();
        let mut advanced = false;
        while let Some(&byte) = bytes.get(self.index) {
            if byte.is_ascii_whitespace() {
                self.index += 1;
                advanced = true;
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
                advanced = true;
                continue;
            }
            break;
        }
        !self.eof() || advanced
    }

    fn consume_keyword(&mut self, keyword: &str) -> bool {
        let bytes = self.source.as_bytes();
        let end = self.index.saturating_add(keyword.len());
        if self.source[self.index..].starts_with(keyword)
            && bytes
                .get(end)
                .map_or(true, |byte| !is_ident_continue(*byte))
        {
            self.index = end;
            return true;
        }
        false
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.source.as_bytes().get(self.index) == Some(&byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_byte(
        &mut self,
        byte: u8,
        expected: &'static str,
    ) -> std::result::Result<(), ParseError> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            let found = self.peek_token().unwrap_or_else(|| "<eof>".to_owned());
            Err(self.error(ParseErrorKind::UnexpectedToken {
                expected: expected.to_owned(),
                found,
            }))
        }
    }

    fn peek_token(&self) -> Option<String> {
        self.source[self.index..]
            .chars()
            .next()
            .map(|ch| ch.to_string())
    }

    fn eof(&self) -> bool {
        self.index >= self.source.len()
    }

    fn error(&self, kind: ParseErrorKind) -> ParseError {
        let (line, column) = line_col_at(self.source, self.index);
        ParseError {
            path: self.path.clone(),
            line,
            column,
            kind,
        }
    }
}

fn line_col_at(source: &str, index: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    for byte in source.as_bytes().iter().take(index) {
        if *byte == b'\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_uppercase() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

pub(crate) fn last_path_segment(path: &str) -> &str {
    path.rsplit(['/', '.']).next().unwrap_or(path)
}
