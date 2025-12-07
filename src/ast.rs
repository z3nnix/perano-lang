#[derive(Debug, Clone)]
pub struct Program {
    #[allow(dead_code)]
    pub package: String,
    pub imports: Vec<Import>,
    pub functions: Vec<Function>,
    pub modules: std::collections::HashMap<String, Module>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    #[allow(dead_code)]
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Module {
    #[allow(dead_code)]
    pub name: String,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    #[allow(dead_code)]
    pub return_type: Option<String>,
    pub body: Vec<Statement>,
    pub is_exported: bool,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    #[allow(dead_code)]
    pub param_type: String,
}

#[derive(Debug, Clone)]
pub enum Statement {
    VarDecl {
        name: String,
        #[allow(dead_code)]
        var_type: Option<String>,
        value: Option<Expression>,
    },
    ArrayDecl {
        name: String,
        #[allow(dead_code)]
        element_type: String,
        size: usize,
    },
    Assignment {
        name: String,
        value: Expression,
    },
    ArrayAssignment {
        name: String,
        index: Expression,
        value: Expression,
    },
    PointerAssignment {
        target: Expression,
        value: Expression,
    },
    If {
        condition: Expression,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    For {
        #[allow(dead_code)]
        init: Option<Box<Statement>>,
        condition: Option<Expression>,
        #[allow(dead_code)]
        post: Option<Box<Statement>>,
        body: Vec<Statement>,
    },
    Return(Option<Expression>),
    Expression(Expression),
}

#[derive(Debug, Clone)]
pub enum Expression {
    Number(i64),
    String(String),
    Identifier(String),
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expression>,
    },
    Call {
        function: String,
        args: Vec<Expression>,
    },
    ModuleCall {
        module: String,
        function: String,
        args: Vec<Expression>,
    },
    ArrayAccess {
        name: String,
        index: Box<Expression>,
    },
    StringIndex {
        string: Box<Expression>,
        index: Box<Expression>,
    },
    AddressOf {
        operand: Box<Expression>,
    },
    Deref {
        operand: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Concat,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}