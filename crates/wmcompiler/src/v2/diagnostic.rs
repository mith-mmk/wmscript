use super::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub path: String,
    pub span: Span,
    pub label: Option<String>,
}

impl Diagnostic {
    pub fn error(
        code: &'static str,
        path: impl Into<String>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            path: path.into(),
            span,
            label: None,
        }
    }
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl core::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}:{}..{}: {}: {}",
            self.path, self.span.start, self.span.end, self.code, self.message
        )
    }
}
