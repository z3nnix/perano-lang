use crate::lexer::Token;
use crate::ast::*;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    file: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, file: &str) -> Self {
        Parser {
            tokens,
            position: 0,
            file: file.to_string(),
        }
    }

    fn current_token(&self) -> &Token {
        if self.position < self.tokens.len() {
            &self.tokens[self.position]
        } else {
            &Token::Eof
        }
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current_token(), Token::Newline) {
            self.advance();
        }
    }

    fn expect(&mut self, expected: Token) -> crate::error::Result<()> {
        if self.current_token() != &expected {
            return Err(crate::error::CompileError::new(
                crate::error::ErrorKind::ParserError,
                format!("expected {:?}, found {:?}", expected, self.current_token()),
                self.file.clone(),
                1,
                1,
            ));
        }
        self.advance();
        Ok(())
    }

    fn error(&self, message: String) -> crate::error::CompileError {
        crate::error::CompileError::new(
            crate::error::ErrorKind::ParserError,
            message,
            self.file.clone(),
            1,
            1,
        )
    }

    pub fn parse(&mut self) -> crate::error::Result<Program> {
        self.skip_newlines();

        self.expect(Token::Package)?;
        let package = if let Token::Identifier(name) = self.current_token() {
            let p = name.clone();
            self.advance();
            p
        } else {
            return Err(self.error("expected package name".to_string()));
        };

        self.skip_newlines();

        let mut imports = Vec::new();
        while matches!(self.current_token(), Token::Import) {
            self.advance();
            if let Token::String(path) = self.current_token() {
                let import = Import {
                    path: path.clone(),
                    alias: None,
                };
                imports.push(import);
                self.advance();
            } else {
                return Err(self.error("expected import path string".to_string()));
            }
            self.skip_newlines();
        }

        let mut functions = Vec::new();
        while !matches!(self.current_token(), Token::Eof) {
            self.skip_newlines();
            if matches!(self.current_token(), Token::Eof) {
                break;
            }
            functions.push(self.parse_function()?);
        }

        Ok(Program {
            package,
            imports,
            functions,
            modules: std::collections::HashMap::new(),
        })
    }

    fn parse_function(&mut self) -> crate::error::Result<Function> {
        let is_pub = if let Token::Identifier(id) = self.current_token() {
            if id == "pub" {
                self.advance();
                true
            } else {
                false
            }
        } else {
            false
        };

        self.expect(Token::Func)?;

        let name = if let Token::Identifier(n) = self.current_token() {
            let name = n.clone();
            self.advance();
            name
        } else {
            return Err(self.error("expected function name".to_string()));
        };

        self.expect(Token::LeftParen)?;
        let mut params = Vec::new();

        while !matches!(self.current_token(), Token::RightParen) {
            let param_name = if let Token::Identifier(n) = self.current_token() {
                let name = n.clone();
                self.advance();
                name
            } else {
                return Err(self.error("expected parameter name".to_string()));
            };

            if matches!(self.current_token(), Token::Colon) {
                self.advance();
            }

            let param_type = if let Token::Identifier(t) = self.current_token() {
                let ty = t.clone();
                self.advance();
                ty
            } else {
                return Err(self.error("expected parameter type".to_string()));
            };

            params.push(Parameter {
                name: param_name,
                param_type,
            });

            if matches!(self.current_token(), Token::Comma) {
                self.advance();
            }
        }

        self.expect(Token::RightParen)?;

        let return_type = if matches!(self.current_token(), Token::Arrow) {
            self.advance();
            if let Token::Identifier(t) = self.current_token() {
                let ty = t.clone();
                self.advance();
                Some(ty)
            } else {
                None
            }
        } else if let Token::Identifier(t) = self.current_token() {
            let ty = t.clone();
            self.advance();
            Some(ty)
        } else {
            None
        };

        self.skip_newlines();
        self.expect(Token::LeftBrace)?;
        self.skip_newlines();

        let mut body = Vec::new();
        while !matches!(self.current_token(), Token::RightBrace) {
            body.push(self.parse_statement()?);
            self.skip_newlines();
        }

        self.expect(Token::RightBrace)?;
        self.skip_newlines();

        let is_exported = is_pub || name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

        Ok(Function {
            name,
            params,
            return_type,
            body,
            is_exported,
        })
    }

    fn parse_statement(&mut self) -> crate::error::Result<Statement> {
        match self.current_token() {
            Token::Var => self.parse_var_decl(),
            Token::If => self.parse_if(),
            Token::For => self.parse_for(),
            Token::Return => self.parse_return(),
            Token::Identifier(_) => {
                let next_pos = self.position + 1;
                if next_pos < self.tokens.len() && (matches!(self.tokens[next_pos], Token::Assign) || matches!(self.tokens[next_pos], Token::LBracket)) {
                    self.parse_assignment()
                } else {
                    Ok(Statement::Expression(self.parse_expression()))
                }
            }
            _ => Ok(Statement::Expression(self.parse_expression())),
        }
    }

    fn parse_var_decl(&mut self) -> crate::error::Result<Statement> {
        self.expect(Token::Var)?;

        let name = if let Token::Identifier(n) = self.current_token() {
            let name = n.clone();
            self.advance();
            name
        } else {
            return Err(self.error("expected variable name".to_string()));
        };

        if matches!(self.current_token(), Token::Colon) {
            self.advance();
        }

        if matches!(self.current_token(), Token::LBracket) {
            self.advance();

            let size = if let Token::Number(n) = self.current_token() {
                let s = *n as usize;
                self.advance();
                s
            } else {
                return Err(self.error("expected array size".to_string()));
            };

            self.expect(Token::RBracket)?;

            let element_type = if let Token::Identifier(t) = self.current_token() {
                let ty = t.clone();
                self.advance();
                ty
            } else {
                return Err(self.error("expected array element type".to_string()));
            };

            return Ok(Statement::ArrayDecl { name, element_type, size });
        }

        let var_type = if let Token::Identifier(t) = self.current_token() {
            let ty = t.clone();
            self.advance();
            Some(ty)
        } else {
            None
        };

        let value = if matches!(self.current_token(), Token::Assign) {
            self.advance();
            Some(self.parse_expression())
        } else {
            None
        };

        Ok(Statement::VarDecl { name, var_type, value })
    }

    fn parse_assignment(&mut self) -> crate::error::Result<Statement> {
        let name = if let Token::Identifier(n) = self.current_token() {
            let name = n.clone();
            self.advance();
            name
        } else {
            return Err(self.error("expected variable name".to_string()));
        };

        if matches!(self.current_token(), Token::LBracket) {
            self.advance();
            let index = self.parse_expression();
            self.expect(Token::RBracket)?;
            self.expect(Token::Assign)?;
            let value = self.parse_expression();

            return Ok(Statement::ArrayAssignment { name, index, value });
        }

        self.expect(Token::Assign)?;
        let value = self.parse_expression();

        Ok(Statement::Assignment { name, value })
    }

    fn parse_if(&mut self) -> crate::error::Result<Statement> {
        self.expect(Token::If)?;

        let condition = self.parse_expression();

        self.skip_newlines();
        self.expect(Token::LeftBrace)?;
        self.skip_newlines();

        let mut then_body = Vec::new();
        while !matches!(self.current_token(), Token::RightBrace) {
            then_body.push(self.parse_statement()?);
            self.skip_newlines();
        }

        self.expect(Token::RightBrace)?;
        self.skip_newlines();

        let else_body = if matches!(self.current_token(), Token::Else) {
            self.advance();
            self.skip_newlines();
            self.expect(Token::LeftBrace)?;
            self.skip_newlines();

            let mut body = Vec::new();
            while !matches!(self.current_token(), Token::RightBrace) {
                body.push(self.parse_statement()?);
                self.skip_newlines();
            }

            self.expect(Token::RightBrace)?;
            Some(body)
        } else {
            None
        };

        Ok(Statement::If {
            condition,
            then_body,
            else_body,
        })
    }

    fn parse_for(&mut self) -> crate::error::Result<Statement> {
        self.expect(Token::For)?;

        let condition = if matches!(self.current_token(), Token::LeftBrace) {
            None
        } else {
            Some(self.parse_expression())
        };

        self.skip_newlines();
        self.expect(Token::LeftBrace)?;
        self.skip_newlines();

        let mut body = Vec::new();
        while !matches!(self.current_token(), Token::RightBrace) {
            body.push(self.parse_statement()?);
            self.skip_newlines();
        }

        self.expect(Token::RightBrace)?;

        Ok(Statement::For {
            init: None,
            condition,
            post: None,
            body,
        })
    }

    fn parse_return(&mut self) -> crate::error::Result<Statement> {
        self.expect(Token::Return)?;

        let value = if matches!(self.current_token(), Token::Newline | Token::RightBrace) {
            None
        } else {
            Some(self.parse_expression())
        };

        Ok(Statement::Return(value))
    }

    fn parse_expression(&mut self) -> Expression {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expression {
        let mut left = self.parse_and();

        while matches!(self.current_token(), Token::Or) {
            self.advance();
            let right = self.parse_and();
            left = Expression::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_and(&mut self) -> Expression {
        let mut left = self.parse_equality();

        while matches!(self.current_token(), Token::And) {
            self.advance();
            let right = self.parse_equality();
            left = Expression::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_equality(&mut self) -> Expression {
        let mut left = self.parse_comparison();

        loop {
            let op = match self.current_token() {
                Token::Equal => BinaryOp::Equal,
                Token::NotEqual => BinaryOp::NotEqual,
                _ => break,
            };

            self.advance();
            let right = self.parse_comparison();
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_comparison(&mut self) -> Expression {
        let mut left = self.parse_additive();

        loop {
            let op = match self.current_token() {
                Token::Less => BinaryOp::Less,
                Token::LessEqual => BinaryOp::LessEqual,
                Token::Greater => BinaryOp::Greater,
                Token::GreaterEqual => BinaryOp::GreaterEqual,
                _ => break,
            };

            self.advance();
            let right = self.parse_additive();
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_additive(&mut self) -> Expression {
        let mut left = self.parse_multiplicative();

        loop {
            let op = match self.current_token() {
                Token::Plus => BinaryOp::Add,
                Token::Minus => BinaryOp::Sub,
                Token::DoublePlus => BinaryOp::Concat,
                _ => break,
            };

            self.advance();
            let right = self.parse_multiplicative();
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_multiplicative(&mut self) -> Expression {
        let mut left = self.parse_unary();

        loop {
            let op = match self.current_token() {
                Token::Star => BinaryOp::Mul,
                Token::Slash => BinaryOp::Div,
                Token::Percent => BinaryOp::Mod,
                _ => break,
            };

            self.advance();
            let right = self.parse_unary();
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_unary(&mut self) -> Expression {
        match self.current_token() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary();
                Expression::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                }
            }
            Token::Not => {
                self.advance();
                let operand = self.parse_unary();
                Expression::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                }
            }
            Token::Ampersand => {
                self.advance();
                let operand = self.parse_unary();
                Expression::AddressOf {
                    operand: Box::new(operand),
                }
            }
            Token::Star => {
                self.advance();
                let operand = self.parse_unary();
                Expression::Deref {
                    operand: Box::new(operand),
                }
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Expression {
        match self.current_token().clone() {
            Token::Number(n) => {
                self.advance();
                Expression::Number(n)
            }
            Token::String(s) => {
                self.advance();

                if matches!(self.current_token(), Token::LBracket) {
                    self.advance();
                    let index = self.parse_expression();
                    if let Err(_) = self.expect(Token::RBracket) {
                        panic!("Expected closing bracket in string index");
                    }

                    return Expression::StringIndex {
                        string: Box::new(Expression::String(s)),
                        index: Box::new(index),
                    };
                }

                Expression::String(s)
            }
            Token::Identifier(name) => {
                self.advance();

                if matches!(self.current_token(), Token::Dot) {
                    self.advance();
                    if let Token::Identifier(func_name) = self.current_token() {
                        let func_name = func_name.clone();
                        self.advance();

                        if matches!(self.current_token(), Token::LeftParen) {
                            self.advance();
                            let mut args = Vec::new();

                            while !matches!(self.current_token(), Token::RightParen) {
                                args.push(self.parse_expression());

                                if matches!(self.current_token(), Token::Comma) {
                                    self.advance();
                                }
                            }

                            if let Err(_) = self.expect(Token::RightParen) {
                                panic!("Expected closing parenthesis in module call");
                            }

                            return Expression::ModuleCall {
                                module: name,
                                function: func_name,
                                args,
                            };
                        }
                    }
                    panic!("Expected function name after module.");
                }

                if matches!(self.current_token(), Token::LeftParen) {
                    self.advance();
                    let mut args = Vec::new();

                    while !matches!(self.current_token(), Token::RightParen) {
                        args.push(self.parse_expression());

                        if matches!(self.current_token(), Token::Comma) {
                            self.advance();
                        }
                    }

                    if let Err(_) = self.expect(Token::RightParen) {
                        panic!("Expected closing parenthesis in function call");
                    }

                    Expression::Call {
                        function: name,
                        args,
                    }
                } else if matches!(self.current_token(), Token::LBracket) {
                    self.advance();
                    let index = self.parse_expression();
                    if let Err(_) = self.expect(Token::RBracket) {
                        panic!("Expected closing bracket in array access");
                    }

                    Expression::ArrayAccess {
                        name,
                        index: Box::new(index),
                    }
                } else {
                    Expression::Identifier(name)
                }
            }
            Token::LeftParen => {
                self.advance();
                let expr = self.parse_expression();
                if let Err(_) = self.expect(Token::RightParen) {
                    panic!("Expected closing parenthesis");
                }
                expr
            }
            _ => panic!("Unexpected token: {:?}", self.current_token()),
        }
    }
}