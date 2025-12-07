use crate::ast::*;
use std::collections::HashMap;

pub struct CodeGen<'a> {
    code: Vec<u8>,
    data: Vec<u8>,
    variables: HashMap<String, i32>,
    stack_offset: i32,
    #[allow(dead_code)]
    string_literals: Vec<(usize, String)>,
    target: String,
    program: Option<&'a Program>,
    in_main: bool,
}

impl<'a> CodeGen<'a> {
    pub fn new(target: &str) -> Self {
        CodeGen {
            code: Vec::new(),
            data: Vec::new(),
            variables: HashMap::new(),
            stack_offset: 0,
            string_literals: Vec::new(),
            target: target.to_string(),
            program: None,
            in_main: false,
        }
    }

    pub fn generate(&mut self, program: &'a Program) -> MachineCode {
        self.program = Some(program);
        self.in_main = true;

        let main_func = program.functions.iter()
            .find(|f| f.name == "main")
            .expect("No main function found");

        if self.target == "elf" {
            self.emit(&[0x55]);
            self.emit(&[0x48, 0x89, 0xE5]);

            for stmt in &main_func.body {
                self.generate_statement(stmt);
            }
            self.emit_exit_with_rax();
        } else {
            self.emit(&[0x55]);
            self.emit(&[0x48, 0x89, 0xE5]);
            self.emit(&[0x48, 0x83, 0xEC, 0x40]);

            for stmt in &main_func.body {
                self.generate_statement(stmt);
            }

            self.emit_exit(0);
        }

        MachineCode {
            code: self.code.clone(),
            data: self.data.clone(),
            entry_point: 0,
        }
    }

    #[allow(dead_code)]
    fn generate_function(&mut self, func: &Function) {
        self.emit(&[0x55]);
        self.emit(&[0x48, 0x89, 0xE5]);

        self.emit(&[0x48, 0x83, 0xEC, 0x40]);

        for stmt in &func.body {
            self.generate_statement(stmt);
        }

        self.emit(&[0x48, 0x89, 0xEC]);
        self.emit(&[0x5D]);
        self.emit(&[0xC3]);
    }

    fn generate_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::VarDecl { name, var_type: _, value } => {
                if let Some(expr) = value {
                    self.generate_expression(expr);
                    self.stack_offset -= 8;
                    self.variables.insert(name.clone(), self.stack_offset);
                    self.emit(&[0x48, 0x89, 0x85]);
                    self.emit_i32(self.stack_offset);
                }
            }
            Statement::ArrayDecl { name, element_type: _, size } => {
                let array_size = (*size as i32) * 8;
                self.stack_offset -= array_size;
                self.variables.insert(name.clone(), self.stack_offset);
                for i in 0..*size {
                    let offset = self.stack_offset + (i as i32 * 8);
                    self.emit(&[0x48, 0xC7, 0x85]);
                    self.emit_i32(offset);
                    self.emit_i32(0);
                }
            }
            Statement::ArrayAssignment { name, index, value } => {
                self.generate_expression(value);
                self.emit(&[0x50]);

                self.generate_expression(index);

                if let Some(&base_offset) = self.variables.get(name) {
                    self.emit(&[0x48, 0x6B, 0xC0, 0x08]);
                    if base_offset >= -128 && base_offset < 128 {
                        self.emit(&[0x48, 0x83, 0xC0, (base_offset as u8)]);
                    } else {
                        self.emit(&[0x48, 0x05]);
                        self.emit_i32(base_offset);
                    }
                    self.emit(&[0x48, 0x01, 0xE8]);

                    self.emit(&[0x59]);
                    self.emit(&[0x48, 0x89, 0x08]);
                }
            }
            Statement::Assignment { name, value } => {
                self.generate_expression(value);
                if let Some(&offset) = self.variables.get(name) {
                    self.emit(&[0x48, 0x89, 0x85]);
                    self.emit_i32(offset);
                }
            }
            Statement::PointerAssignment { target, value } => {
                // Generate value first
                self.generate_expression(value);
                self.emit(&[0x50]); // push rax (save value)
                
                // Generate target address
                self.generate_expression(target);
                
                // Pop value and store through pointer
                self.emit(&[0x59]); // pop rcx (restore value)
                self.emit(&[0x48, 0x89, 0x08]); // mov [rax], rcx
            }
            Statement::Return(expr) => {
                if let Some(e) = expr {
                    self.generate_expression(e);
                } else {
                    self.emit(&[0x48, 0x31, 0xC0]);
                }

                if self.in_main {
                    if self.target == "elf" {
                        self.emit_exit_with_rax();
                    } else {
                        self.emit(&[0x89, 0xC1]);
                        self.emit(&[0x48, 0x83, 0xEC, 0x20]);
                        self.emit(&[0xFF, 0x15]);
                        self.emit_i32(0x10000000u32 as i32);
                    }
                }
            }
            Statement::Expression(expr) => {
                self.generate_expression(expr);
            }
            Statement::If { condition, then_body, else_body } => {
                self.generate_expression(condition);

                self.emit(&[0x48, 0x85, 0xC0]);

                self.emit(&[0x0F, 0x84]);
                let else_jump_pos = self.code.len();
                self.emit_i32(0);

                for stmt in then_body {
                    self.generate_statement(stmt);
                }

                self.emit(&[0xE9]);
                let end_jump_pos = self.code.len();
                self.emit_i32(0);

                let else_label = self.code.len();
                let else_offset = (else_label as i32) - (else_jump_pos as i32) - 4;
                self.patch_i32(else_jump_pos, else_offset);

                if let Some(body) = else_body {
                    for stmt in body {
                        self.generate_statement(stmt);
                    }
                }

                let end_label = self.code.len();
                let end_offset = (end_label as i32) - (end_jump_pos as i32) - 4;
                self.patch_i32(end_jump_pos, end_offset);
            }
            Statement::For { init: _, condition, post: _, body } => {
                let loop_start = self.code.len();

                if let Some(cond) = condition {
                    self.generate_expression(cond);
                    self.emit(&[0x48, 0x85, 0xC0]);
                    self.emit(&[0x0F, 0x84]);
                    let end_jump_pos = self.code.len();
                    self.emit_i32(0);

                    for stmt in body {
                        self.generate_statement(stmt);
                    }

                    self.emit(&[0xE9]);
                    let back_offset = (loop_start as i32) - (self.code.len() as i32) - 4;
                    self.emit_i32(back_offset);

                    let end_label = self.code.len();
                    let end_offset = (end_label as i32) - (end_jump_pos as i32) - 4;
                    self.patch_i32(end_jump_pos, end_offset);
                } else {
                    for stmt in body {
                        self.generate_statement(stmt);
                    }

                    self.emit(&[0xE9]);
                    let back_offset = (loop_start as i32) - (self.code.len() as i32) - 4;
                    self.emit_i32(back_offset);
                }
            }
        }
    }

    fn generate_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::Number(n) => {
                self.emit(&[0x48, 0xB8]);
                self.emit_i64(*n);
            }
            Expression::Identifier(name) => {
                if let Some(&offset) = self.variables.get(name) {
                    self.emit(&[0x48, 0x8B, 0x85]);
                    self.emit_i32(offset);
                }
            }
            Expression::Binary { op, left, right } => {
                self.generate_expression(right);
                self.emit(&[0x50]);

                self.generate_expression(left);
                self.emit(&[0x59]);

                match op {
                    BinaryOp::Add => {
                        self.emit(&[0x48, 0x01, 0xC8]);
                    }
                    BinaryOp::Sub => {
                        self.emit(&[0x48, 0x29, 0xC8]);
                    }
                    BinaryOp::Mul => {
                        self.emit(&[0x48, 0x0F, 0xAF, 0xC1]);
                    }
                    BinaryOp::Div => {
                        self.emit(&[0x48, 0x31, 0xD2]);
                        self.emit(&[0x48, 0xF7, 0xF9]);
                    }
                    BinaryOp::Equal => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x94, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::NotEqual => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x95, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::Less => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x9C, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::LessEqual => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x9E, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::Greater => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x9F, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::GreaterEqual => {
                        self.emit(&[0x48, 0x39, 0xC8]);
                        self.emit(&[0x0F, 0x9D, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                    BinaryOp::Mod => {
                        self.emit(&[0x48, 0x31, 0xD2]);
                        self.emit(&[0x48, 0xF7, 0xF9]);
                        self.emit(&[0x48, 0x89, 0xD0]);
                    }
                    BinaryOp::Concat => {
                    }
                    _ => {}
                }
            }
            Expression::Unary { op, operand } => {
                self.generate_expression(operand);
                match op {
                    UnaryOp::Neg => {
                        self.emit(&[0x48, 0xF7, 0xD8]);
                    }
                    UnaryOp::Not => {
                        self.emit(&[0x48, 0x85, 0xC0]);
                        self.emit(&[0x0F, 0x94, 0xC0]);
                        self.emit(&[0x48, 0x0F, 0xB6, 0xC0]);
                    }
                }
            }
            Expression::ArrayAccess { name, index } => {
                self.generate_expression(index);

                if let Some(&base_offset) = self.variables.get(name) {
                    self.emit(&[0x48, 0x6B, 0xC0, 0x08]);
                    if base_offset >= -128 && base_offset < 128 {
                        self.emit(&[0x48, 0x83, 0xC0, (base_offset as u8)]);
                    } else {
                        self.emit(&[0x48, 0x05]);
                        self.emit_i32(base_offset);
                    }
                    self.emit(&[0x48, 0x01, 0xE8]);

                    self.emit(&[0x48, 0x8B, 0x00]);
                }
            }
            Expression::Call { function, args } => {
                if function == "exit" {
                    self.emit_exit(0);
                } else if function == "println" {
                    if !args.is_empty() {
                        match &args[0] {
                            Expression::String(s) => {
                                self.emit_println(s);
                            }
                            _ => {
                                self.generate_expression(&args[0]);
                                self.emit_println_int();
                            }
                        }
                    }
                } else if function == "len" && args.len() == 1 {
                    if let Expression::String(s) = &args[0] {
                        self.emit(&[0x48, 0xB8]);
                        self.emit_i64(s.len() as i64);
                    } else {
                        self.emit(&[0x48, 0x31, 0xC0]);
                    }
                } else if function == "concat" && args.len() == 2 {
                    self.emit(&[0x48, 0x31, 0xC0]);
                } else if function == "compare" && args.len() == 2 {
                    if let Expression::String(s1) = &args[0] {
                        if let Expression::String(s2) = &args[1] {
                            let result = if s1 == s2 { 0 } else if s1 < s2 { -1 } else { 1 };
                            self.emit(&[0x48, 0xB8]);
                            self.emit_i64(result);
                        } else {
                            self.emit(&[0x48, 0x31, 0xC0]);
                        }
                    } else {
                        self.emit(&[0x48, 0x31, 0xC0]);
                    }
                } else {
                    self.generate_iperine_call(function, args);
                }
            }
            Expression::ModuleCall { module, function, args } => {
                self.generate_module_call(module, function, args);
            }
            Expression::StringIndex { string, index } => {
                if let Expression::String(_s) = string.as_ref() {
                    self.generate_expression(index);
                }
            }
            Expression::AddressOf { operand } => {
                if let Expression::Identifier(name) = operand.as_ref() {
                    if let Some(&offset) = self.variables.get(name) {
                        self.emit(&[0x48, 0x8D, 0x85]);
                        self.emit_i32(offset);
                    }
                }
            }
            Expression::Deref { operand } => {
                self.generate_expression(operand);
                self.emit(&[0x48, 0x8B, 0x00]);
            }
            _ => {}
        }
    }

    fn emit(&mut self, bytes: &[u8]) {
        self.code.extend_from_slice(bytes);
    }

    fn emit_i32(&mut self, value: i32) {
        self.code.extend_from_slice(&value.to_le_bytes());
    }

    fn emit_i64(&mut self, value: i64) {
        self.code.extend_from_slice(&value.to_le_bytes());
    }

    fn patch_i32(&mut self, pos: usize, value: i32) {
        let bytes = value.to_le_bytes();
        self.code[pos..pos + 4].copy_from_slice(&bytes);
    }

    fn emit_println(&mut self, text: &str) {
        if self.target == "elf" {
            let str_len = text.len() + 1;

            self.emit(&[0xEB]);
            self.emit(&[(str_len as u8) & 0xFF]);

            let string_addr = self.code.len();
            self.code.extend_from_slice(text.as_bytes());
            self.code.push(b'\n');

            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);

            let lea_instr_pos = self.code.len() + 7;
            let offset = (string_addr as i32) - (lea_instr_pos as i32);
            self.emit(&[0x48, 0x8D, 0x35]);
            self.emit_i32(offset);

            self.emit(&[0x48, 0xC7, 0xC2]);
            self.emit_i32(str_len as i32);

            self.emit(&[0x0F, 0x05]);
        } else {
            let str_len = text.len() + 1;

            self.emit(&[0x48, 0x83, 0xEC, 0x38]);

            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            self.emit(&[0x48, 0x89, 0xC3]);

            self.emit(&[0xEB]);
            self.emit(&[(str_len as u8) & 0xFF]);

            let string_addr = self.code.len();
            self.code.extend_from_slice(text.as_bytes());
            self.code.push(b'\n');

            self.emit(&[0x48, 0x89, 0xD9]);

            let lea_instr_pos = self.code.len() + 7;
            let offset = (string_addr as i32) - (lea_instr_pos as i32);
            self.emit(&[0x48, 0x8D, 0x15]);
            self.emit_i32(offset);

            self.emit(&[0x41, 0xB8]);
            self.emit_i32(str_len as i32);

            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x20]);

            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);

            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20080000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x38]);
        }
    }

    fn emit_exit(&mut self, code: i32) {
        if self.target == "elf" {
            self.emit(&[0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7]);
            self.emit_i32(code);
            self.emit(&[0x0F, 0x05]);
        } else {
            self.emit(&[0x48, 0x83, 0xEC, 0x20]);

            self.emit(&[0xB9]);
            self.emit_i32(code);

            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x10000000u32 as i32);
        }
    }

    fn emit_exit_with_rax(&mut self) {
        self.emit(&[0x48, 0x89, 0xC7]);
        self.emit(&[0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00]);
        self.emit(&[0x0F, 0x05]);
    }

    fn emit_println_int(&mut self) {

        if self.target == "elf" {
            self.emit(&[0x48, 0x83, 0xEC, 0x20]);
            self.emit(&[0x48, 0x8D, 0x7C, 0x24, 0x1E]);
            self.emit(&[0xC6, 0x07, 0x0A]);
            self.emit(&[0x48, 0xFF, 0xCF]);

            self.emit(&[0x48, 0x89, 0xC3]);
            self.emit(&[0x48, 0x85, 0xC0]);
            self.emit(&[0x75, 0x05]);
            self.emit(&[0xC6, 0x07, 0x30]);
            self.emit(&[0xEB, 0x29]);

            self.emit(&[0x48, 0x31, 0xC9]);
            self.emit(&[0x48, 0x85, 0xDB]);
            self.emit(&[0x79, 0x0F]);
            self.emit(&[0x48, 0x89, 0xDA]);
            self.emit(&[0x48, 0xC1, 0xFA, 0x3F]);
            self.emit(&[0x48, 0x31, 0xD3]);
            self.emit(&[0x48, 0x29, 0xD3]);
            self.emit(&[0x48, 0xFF, 0xC1]);

            self.emit(&[0x41, 0xB8, 0x0A, 0x00, 0x00, 0x00]);

            let loop_start = self.code.len();
            self.emit(&[0x48, 0x89, 0xD8]);
            self.emit(&[0x48, 0x31, 0xD2]);
            self.emit(&[0x49, 0xF7, 0xF0]);
            self.emit(&[0x80, 0xC2, 0x30]);
            self.emit(&[0x88, 0x17]);
            self.emit(&[0x48, 0xFF, 0xCF]);
            self.emit(&[0x48, 0x89, 0xC3]);
            self.emit(&[0x48, 0x85, 0xC0]);
            let back = (loop_start as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0x75, (back as u8)]);

            self.emit(&[0x48, 0x85, 0xC9]);
            self.emit(&[0x74, 0x03]);
            self.emit(&[0xC6, 0x07, 0x2D]);
            self.emit(&[0x48, 0xFF, 0xCF]);

            self.emit(&[0x48, 0xFF, 0xC7]);
            self.emit(&[0x48, 0x8D, 0x74, 0x24, 0x20]);
            self.emit(&[0x48, 0x29, 0xFE]);
            self.emit(&[0x48, 0x89, 0xF2]);
            self.emit(&[0x48, 0x89, 0xFE]);
            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x0F, 0x05]);
            self.emit(&[0x48, 0x83, 0xC4, 0x20]);
        } else {
            self.emit(&[0x48, 0x83, 0xEC, 0x60]);

            self.emit(&[0x48, 0x8D, 0x4C, 0x24, 0x5E]);
            self.emit(&[0xC6, 0x01, 0x0A]);
            self.emit(&[0x48, 0xFF, 0xC9]);

            self.emit(&[0x48, 0x85, 0xC0]);
            self.emit(&[0x0F, 0x85]);
            let not_zero_patch = self.code.len();
            self.emit_i32(0);

            self.emit(&[0xC6, 0x01, 0x30]);
            self.emit(&[0xE9]);
            let done_patch1 = self.code.len();
            self.emit_i32(0);

            let not_zero_pos = self.code.len();
            self.patch_i32(not_zero_patch, (not_zero_pos as i32) - (not_zero_patch as i32) - 4);

            self.emit(&[0x48, 0x89, 0xC2]);
            self.emit(&[0x48, 0xC1, 0xFA, 0x3F]);
            self.emit(&[0x48, 0x31, 0xD0]);
            self.emit(&[0x48, 0x29, 0xD0]);

            self.emit(&[0x4C, 0x89, 0xD3]);
            self.emit(&[0x49, 0x89, 0xD3]);

            self.emit(&[0x41, 0xB8, 0x0A, 0x00, 0x00, 0x00]);
            let loop_pos = self.code.len();
            self.emit(&[0x48, 0x31, 0xD2]);
            self.emit(&[0x49, 0xF7, 0xF0]);
            self.emit(&[0x80, 0xC2, 0x30]);
            self.emit(&[0x88, 0x11]);
            self.emit(&[0x48, 0xFF, 0xC9]);
            self.emit(&[0x48, 0x85, 0xC0]);
            let loop_back = (loop_pos as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0x75, (loop_back as u8)]);

            self.emit(&[0x4D, 0x85, 0xDB]);
            self.emit(&[0x79, 0x03]);
            self.emit(&[0xC6, 0x01, 0x2D]);
            self.emit(&[0x48, 0xFF, 0xC9]);

            self.emit(&[0x4C, 0x89, 0xDA]);
            let done_pos = self.code.len();
            self.patch_i32(done_patch1, (done_pos as i32) - (done_patch1 as i32) - 4);

            self.emit(&[0x48, 0xFF, 0xC1]);

            self.emit(&[0x48, 0x8D, 0x44, 0x24, 0x60]);
            self.emit(&[0x48, 0x29, 0xC8]);

            self.emit(&[0x48, 0x89, 0x4C, 0x24, 0x28]);
            self.emit(&[0x48, 0x89, 0x44, 0x24, 0x30]);

            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            self.emit(&[0x48, 0x89, 0xC1]);
            self.emit(&[0x48, 0x8B, 0x54, 0x24, 0x28]);
            self.emit(&[0x4C, 0x8B, 0x44, 0x24, 0x30]);
            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]);
            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20080000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x60]);
        }
    }

    fn generate_iperine_call(&mut self, function: &str, args: &[Expression]) {
        let saved_vars = self.variables.clone();
        let saved_offset = self.stack_offset;
        let saved_in_main = self.in_main;
        self.in_main = false;

        if let Some(prog) = self.program {
            if let Some(func) = prog.functions.iter().find(|f| f.name == function) {
                for (i, arg) in args.iter().enumerate() {
                    if i < func.params.len() {
                        self.generate_expression(arg);
                        self.stack_offset -= 8;
                        self.variables.insert(func.params[i].name.clone(), self.stack_offset);
                        self.emit(&[0x48, 0x89, 0x85]);
                        self.emit_i32(self.stack_offset);
                    }
                }

                for stmt in &func.body {
                    self.generate_statement(stmt);
                }
            }
        }

        self.variables = saved_vars;
        self.stack_offset = saved_offset;
        self.in_main = saved_in_main;
    }

    fn generate_stdio_println(&mut self, value: &Expression) {
        self.generate_expression(value);
        self.emit_println_int();
    }

    fn emit_print_int(&mut self) {
        if self.target == "elf" {
            self.emit(&[0x48, 0x83, 0xEC, 0x20]);
            self.emit(&[0x48, 0x8D, 0x7C, 0x24, 0x1E]);
            self.emit(&[0xC6, 0x07, 0x00]);
            self.emit(&[0x48, 0xFF, 0xCF]);

            self.emit(&[0x48, 0x89, 0xC3]);
            self.emit(&[0x48, 0x85, 0xC0]);
            self.emit(&[0x75, 0x05]);
            self.emit(&[0xC6, 0x07, 0x30]);
            self.emit(&[0xEB, 0x29]);

            self.emit(&[0x48, 0x31, 0xC9]);
            self.emit(&[0x48, 0x85, 0xDB]);
            self.emit(&[0x79, 0x0F]);
            self.emit(&[0x48, 0x89, 0xDA]);
            self.emit(&[0x48, 0xC1, 0xFA, 0x3F]);
            self.emit(&[0x48, 0x31, 0xD3]);
            self.emit(&[0x48, 0x29, 0xD3]);
            self.emit(&[0x48, 0xFF, 0xC1]);

            self.emit(&[0x41, 0xB8, 0x0A, 0x00, 0x00, 0x00]);

            let loop_start = self.code.len();
            self.emit(&[0x48, 0x89, 0xD8]);
            self.emit(&[0x48, 0x31, 0xD2]);
            self.emit(&[0x49, 0xF7, 0xF0]);
            self.emit(&[0x80, 0xC2, 0x30]);
            self.emit(&[0x88, 0x17]);
            self.emit(&[0x48, 0xFF, 0xCF]);
            self.emit(&[0x48, 0x89, 0xC3]);
            self.emit(&[0x48, 0x85, 0xC0]);
            let back = (loop_start as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0x75, (back as u8)]);

            self.emit(&[0x48, 0x85, 0xC9]);
            self.emit(&[0x74, 0x03]);
            self.emit(&[0xC6, 0x07, 0x2D]);
            self.emit(&[0x48, 0xFF, 0xCF]);

            self.emit(&[0x48, 0xFF, 0xC7]);
            self.emit(&[0x48, 0x8D, 0x74, 0x24, 0x20]);
            self.emit(&[0x48, 0x29, 0xFE]);
            self.emit(&[0x48, 0x89, 0xF2]);
            self.emit(&[0x48, 0x89, 0xFE]);
            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x0F, 0x05]);
            self.emit(&[0x48, 0x83, 0xC4, 0x20]);
        } else {
            self.emit(&[0x48, 0x83, 0xEC, 0x60]);

            self.emit(&[0x48, 0x8D, 0x4C, 0x24, 0x5E]);
            self.emit(&[0xC6, 0x01, 0x00]);
            self.emit(&[0x48, 0xFF, 0xC9]);

            self.emit(&[0x48, 0x85, 0xC0]);
            self.emit(&[0x0F, 0x85]);
            let not_zero_patch = self.code.len();
            self.emit_i32(0);

            self.emit(&[0xC6, 0x01, 0x30]);
            self.emit(&[0xE9]);
            let done_patch1 = self.code.len();
            self.emit_i32(0);

            let not_zero_pos = self.code.len();
            self.patch_i32(not_zero_patch, (not_zero_pos as i32) - (not_zero_patch as i32) - 4);

            self.emit(&[0x48, 0x89, 0xC2]);
            self.emit(&[0x48, 0xC1, 0xFA, 0x3F]);
            self.emit(&[0x48, 0x31, 0xD0]);
            self.emit(&[0x48, 0x29, 0xD0]);

            self.emit(&[0x4C, 0x89, 0xD3]);
            self.emit(&[0x49, 0x89, 0xD3]);

            self.emit(&[0x41, 0xB8, 0x0A, 0x00, 0x00, 0x00]);
            let loop_pos = self.code.len();
            self.emit(&[0x48, 0x31, 0xD2]);
            self.emit(&[0x49, 0xF7, 0xF0]);
            self.emit(&[0x80, 0xC2, 0x30]);
            self.emit(&[0x88, 0x11]);
            self.emit(&[0x48, 0xFF, 0xC9]);
            self.emit(&[0x48, 0x85, 0xC0]);
            let loop_back = (loop_pos as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0x75, (loop_back as u8)]);

            self.emit(&[0x4D, 0x85, 0xDB]);
            self.emit(&[0x79, 0x03]);
            self.emit(&[0xC6, 0x01, 0x2D]);
            self.emit(&[0x48, 0xFF, 0xC9]);

            self.emit(&[0x4C, 0x89, 0xDA]);
            let done_pos = self.code.len();
            self.patch_i32(done_patch1, (done_pos as i32) - (done_patch1 as i32) - 4);

            self.emit(&[0x48, 0xFF, 0xC1]);

            self.emit(&[0x48, 0x8D, 0x44, 0x24, 0x60]);
            self.emit(&[0x48, 0x29, 0xC8]);

            self.emit(&[0x48, 0x89, 0x4C, 0x24, 0x28]);
            self.emit(&[0x48, 0x89, 0x44, 0x24, 0x30]);

            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            self.emit(&[0x48, 0x89, 0xC1]);
            self.emit(&[0x48, 0x8B, 0x54, 0x24, 0x28]);
            self.emit(&[0x4C, 0x8B, 0x44, 0x24, 0x30]);
            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]);
            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20080000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x60]);
        }
    }

    fn emit_print_str(&mut self, text: &str) {
        if self.target == "elf" {
            let str_len = text.len();

            self.emit(&[0xEB]);
            self.emit(&[(str_len as u8) & 0xFF]);

            let string_addr = self.code.len();
            self.code.extend_from_slice(text.as_bytes());

            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);

            let lea_instr_pos = self.code.len() + 7;
            let offset = (string_addr as i32) - (lea_instr_pos as i32);
            self.emit(&[0x48, 0x8D, 0x35]);
            self.emit_i32(offset);

            self.emit(&[0x48, 0xC7, 0xC2]);
            self.emit_i32(str_len as i32);

            self.emit(&[0x0F, 0x05]);
        } else {
            let str_len = text.len();

            self.emit(&[0x48, 0x83, 0xEC, 0x38]);

            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            self.emit(&[0x48, 0x89, 0xC3]);

            self.emit(&[0xEB]);
            self.emit(&[(str_len as u8) & 0xFF]);

            let string_addr = self.code.len();
            self.code.extend_from_slice(text.as_bytes());

            self.emit(&[0x48, 0x89, 0xD9]);

            let lea_instr_pos = self.code.len() + 7;
            let offset = (string_addr as i32) - (lea_instr_pos as i32);
            self.emit(&[0x48, 0x8D, 0x15]);
            self.emit_i32(offset);

            self.emit(&[0x41, 0xB8]);
            self.emit_i32(str_len as i32);

            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x20]);

            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);

            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20080000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x38]);
        }
    }

    fn emit_print_char(&mut self) {
        if self.target == "elf" {
            // Linux: write(1, &char, 1)
            self.emit(&[0x48, 0x83, 0xEC, 0x10]);
            self.emit(&[0x88, 0x04, 0x24]);
            self.emit(&[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x48, 0x89, 0xE6]);
            self.emit(&[0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x0F, 0x05]);
            self.emit(&[0x48, 0x83, 0xC4, 0x10]);
        } else {
            
            self.emit(&[0x48, 0x83, 0xEC, 0x48]);
            self.emit(&[0x88, 0x44, 0x24, 0x30]); // store char on stack

            
            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            
            self.emit(&[0x48, 0x89, 0xC1]);
            self.emit(&[0x48, 0x8D, 0x54, 0x24, 0x30]);
            self.emit(&[0x41, 0xB8, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]);
            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20080000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x48]);
        }
    }

    fn emit_read_int(&mut self) {
        // Read integer from stdin, return in RAX
        if self.target == "elf" {
            // Use scanf-like approach with read syscall
            self.emit(&[0x48, 0x83, 0xEC, 0x20]);
            
            // Read up to 20 bytes from stdin
            self.emit(&[0x48, 0x31, 0xC0]); // mov rax, 0 (read)
            self.emit(&[0x48, 0x31, 0xFF]); // mov rdi, 0 (stdin)
            self.emit(&[0x48, 0x89, 0xE6]); // mov rsi, rsp
            self.emit(&[0x48, 0xC7, 0xC2, 0x14, 0x00, 0x00, 0x00]); // mov rdx, 20
            self.emit(&[0x0F, 0x05]); // syscall

            // Parse integer from buffer
            self.emit(&[0x48, 0x31, 0xC0]); // result = 0
            self.emit(&[0x48, 0x31, 0xC9]); // sign = 0
            self.emit(&[0x48, 0x89, 0xE6]); // ptr = rsp

            // Check for minus sign
            self.emit(&[0x80, 0x3E, 0x2D]); // cmp byte [rsi], '-'
            self.emit(&[0x75, 0x07]); // jne skip_sign
            self.emit(&[0x48, 0xFF, 0xC1]); // inc rcx (sign = 1)
            self.emit(&[0x48, 0xFF, 0xC6]); // inc rsi (skip '-')

            // Parse loop
            let loop_start = self.code.len();
            self.emit(&[0x0F, 0xB6, 0x1E]); // movzx ebx, byte [rsi]
            self.emit(&[0x80, 0xFB, 0x30]); // cmp bl, '0'
            self.emit(&[0x72, 0x13]); // jb done
            self.emit(&[0x80, 0xFB, 0x39]); // cmp bl, '9'
            self.emit(&[0x77, 0x0F]); // ja done
            
            self.emit(&[0x48, 0x6B, 0xC0, 0x0A]); // imul rax, 10
            self.emit(&[0x80, 0xEB, 0x30]); // sub bl, '0'
            self.emit(&[0x48, 0x0F, 0xB6, 0xDB]); // movzx rbx, bl
            self.emit(&[0x48, 0x01, 0xD8]); // add rax, rbx
            self.emit(&[0x48, 0xFF, 0xC6]); // inc rsi
            let back = (loop_start as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0xEB, (back as u8)]); // jmp loop_start

            // Apply sign
            self.emit(&[0x48, 0x85, 0xC9]); // test rcx, rcx
            self.emit(&[0x74, 0x03]); // jz skip_neg
            self.emit(&[0x48, 0xF7, 0xD8]); // neg rax

            self.emit(&[0x48, 0x83, 0xC4, 0x20]);
        } else {
            // Windows: use scanf simulation
            self.emit(&[0x48, 0x83, 0xEC, 0x48]);

            // GetStdHandle(-10) for stdin
            self.emit(&[0xB9, 0xF6, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            // ReadFile(handle, buffer, 20, &read, NULL)
            self.emit(&[0x48, 0x89, 0xC1]); // handle
            self.emit(&[0x48, 0x8D, 0x54, 0x24, 0x30]); // buffer
            self.emit(&[0x41, 0xB8, 0x14, 0x00, 0x00, 0x00]); // 20 bytes
            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x28]); // &bytes_read
            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20100000u32 as i32);

            // Parse integer
            self.emit(&[0x48, 0x31, 0xC0]); // result = 0
            self.emit(&[0x48, 0x31, 0xC9]); // sign = 0
            self.emit(&[0x48, 0x8D, 0x74, 0x24, 0x30]); // ptr

            // Check for minus
            self.emit(&[0x80, 0x3E, 0x2D]);
            self.emit(&[0x75, 0x07]);
            self.emit(&[0x48, 0xFF, 0xC1]);
            self.emit(&[0x48, 0xFF, 0xC6]);

            // Parse loop
            let loop_start = self.code.len();
            self.emit(&[0x0F, 0xB6, 0x1E]);
            self.emit(&[0x80, 0xFB, 0x30]);
            self.emit(&[0x72, 0x13]);
            self.emit(&[0x80, 0xFB, 0x39]);
            self.emit(&[0x77, 0x0F]);
            
            self.emit(&[0x48, 0x6B, 0xC0, 0x0A]);
            self.emit(&[0x80, 0xEB, 0x30]);
            self.emit(&[0x48, 0x0F, 0xB6, 0xDB]);
            self.emit(&[0x48, 0x01, 0xD8]);
            self.emit(&[0x48, 0xFF, 0xC6]);
            let back = (loop_start as i32) - (self.code.len() as i32) - 2;
            self.emit(&[0xEB, (back as u8)]);

            // Apply sign
            self.emit(&[0x48, 0x85, 0xC9]);
            self.emit(&[0x74, 0x03]);
            self.emit(&[0x48, 0xF7, 0xD8]);

            self.emit(&[0x48, 0x83, 0xC4, 0x48]);
        }
    }

    fn emit_read_char(&mut self) {
        // Read single character from stdin, return in RAX
        if self.target == "elf" {
            self.emit(&[0x48, 0x83, 0xEC, 0x10]);
            
            self.emit(&[0x48, 0x31, 0xC0]); // mov rax, 0 (read)
            self.emit(&[0x48, 0x31, 0xFF]); // mov rdi, 0 (stdin)
            self.emit(&[0x48, 0x89, 0xE6]); // mov rsi, rsp
            self.emit(&[0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00]); // mov rdx, 1
            self.emit(&[0x0F, 0x05]); // syscall

            self.emit(&[0x48, 0x0F, 0xB6, 0x04, 0x24]); // movzx rax, byte [rsp]
            self.emit(&[0x48, 0x83, 0xC4, 0x10]);
        } else {
            self.emit(&[0x48, 0x83, 0xEC, 0x48]);

            // GetStdHandle(-10) for stdin
            self.emit(&[0xB9, 0xF6, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            // ReadFile(handle, &char, 1, &read, NULL)
            self.emit(&[0x48, 0x89, 0xC1]);
            self.emit(&[0x48, 0x8D, 0x54, 0x24, 0x30]);
            self.emit(&[0x41, 0xB8, 0x01, 0x00, 0x00, 0x00]);
            self.emit(&[0x4C, 0x8D, 0x4C, 0x24, 0x38]);
            self.emit(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0x00, 0x00, 0x00, 0x00]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20100000u32 as i32);

            self.emit(&[0x48, 0x0F, 0xB6, 0x44, 0x24, 0x30]); // movzx rax, byte [rsp+0x30]
            self.emit(&[0x48, 0x83, 0xC4, 0x48]);
        }
    }

    fn emit_flush(&mut self) {
        // Flush stdout
        if self.target == "elf" {
            // Linux: fsync(1)
            self.emit(&[0x48, 0xC7, 0xC0, 0x4A, 0x00, 0x00, 0x00]); // mov rax, 74 (fsync)
            self.emit(&[0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00]); // mov rdi, 1 (stdout)
            self.emit(&[0x0F, 0x05]); // syscall
        } else {
            // Windows: FlushFileBuffers
            self.emit(&[0x48, 0x83, 0xEC, 0x28]);

            // GetStdHandle(-11) for stdout
            self.emit(&[0xB9, 0xF5, 0xFF, 0xFF, 0xFF]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20000000u32 as i32);

            // FlushFileBuffers(handle)
            self.emit(&[0x48, 0x89, 0xC1]);
            self.emit(&[0xFF, 0x15]);
            self.emit_i32(0x20180000u32 as i32);

            self.emit(&[0x48, 0x83, 0xC4, 0x28]);
        }
    }

    fn generate_module_call(&mut self, module: &str, function: &str, args: &[Expression]) {
        if module == "stdio" {
            if function == "Println" && args.len() == 1 {
                self.generate_stdio_println(&args[0]);
                return;
            } else if function == "Print" && args.len() == 1 {
                self.generate_expression(&args[0]);
                self.emit_print_int();
                return;
            } else if function == "PrintlnStr" && args.len() == 1 {
                if let Expression::String(s) = &args[0] {
                    self.emit_println(s);
                }
                return;
            } else if function == "PrintStr" && args.len() == 1 {
                if let Expression::String(s) = &args[0] {
                    self.emit_print_str(s);
                }
                return;
            } else if function == "PrintChar" && args.len() == 1 {
                self.generate_expression(&args[0]);
                self.emit_print_char();
                return;
            } else if function == "ReadInt" && args.is_empty() {
                self.emit_read_int();
                return;
            } else if function == "ReadChar" && args.is_empty() {
                self.emit_read_char();
                return;
            } else if function == "Flush" && args.is_empty() {
                self.emit_flush();
                return;
            }
        }
        let saved_vars = self.variables.clone();
        let saved_offset = self.stack_offset;
        let saved_in_main = self.in_main;
        self.in_main = false;

        if let Some(prog) = self.program {
            if let Some(module_def) = prog.modules.get(module) {
                if let Some(func) = module_def.functions.iter().find(|f| f.name == function) {
                    if !func.is_exported {
                        panic!("Function '{}' is not exported from module '{}'", function, module);
                    }

                    for (i, arg) in args.iter().enumerate() {
                        if i < func.params.len() {
                            self.generate_expression(arg);
                            self.stack_offset -= 8;
                            self.variables.insert(func.params[i].name.clone(), self.stack_offset);
                            self.emit(&[0x48, 0x89, 0x85]);
                            self.emit_i32(self.stack_offset);
                        }
                    }

                    for stmt in &func.body {
                        self.generate_statement(stmt);
                    }
                } else {
                    panic!("Function '{}' not found in module '{}'", function, module);
                }
            } else {
                panic!("Module '{}' not found", module);
            }
        }

        self.variables = saved_vars;
        self.stack_offset = saved_offset;
        self.in_main = saved_in_main;
    }
}

pub struct MachineCode {
    pub code: Vec<u8>,
    #[allow(dead_code)]
    pub data: Vec<u8>,
    #[allow(dead_code)]
    pub entry_point: usize,
}