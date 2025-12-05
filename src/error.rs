use std::fmt;

#[derive(Debug, Clone)]
pub struct CompileError {
    pub kind: ErrorKind,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub source_line: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    LexerError,
    ParserError,
    #[allow(dead_code)]
    TypeError,
    ModuleError,
    #[allow(dead_code)]
    CodeGenError,
}

impl CompileError {
    pub fn new(kind: ErrorKind, message: String, file: String, line: usize, column: usize) -> Self {
        CompileError {
            kind,
            message,
            file,
            line,
            column,
            source_line: None,
        }
    }

    pub fn with_source_line(mut self, source_line: String) -> Self {
        self.source_line = Some(source_line);
        self
    }

    pub fn display(&self) {
        let kind_str = match self.kind {
            ErrorKind::LexerError => "lexer error",
            ErrorKind::ParserError => "parser error",
            ErrorKind::TypeError => "type error",
            ErrorKind::ModuleError => "module error",
            ErrorKind::CodeGenError => "codegen error",
        };

        eprintln!("\x1b[1;31merror\x1b[0m: {}", self.message);
        eprintln!("  \x1b[1;34m-->\x1b[0m {}:{}:{}", self.file, self.line, self.column);

        if let Some(ref source) = self.source_line {
            eprintln!("\x1b[1;34m{:4} |\x1b[0m", self.line);
            eprintln!("\x1b[1;34m     |\x1b[0m {}", source);
            eprintln!("\x1b[1;34m     |\x1b[0m {}\x1b[1;31m^\x1b[0m {}",
                      " ".repeat(self.column.saturating_sub(1)),
                      kind_str);
        }
        eprintln!();
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}:{}: {}", self.file, self.line, self.column, self.message)
    }
}

impl std::error::Error for CompileError {}

pub type Result<T> = std::result::Result<T, CompileError>;