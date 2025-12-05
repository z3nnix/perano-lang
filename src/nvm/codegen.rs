use crate::ast::*;
use std::collections::HashMap;
const HALT: u8 = 0x00;
const NOP: u8 = 0x01;
const PUSH32: u8 = 0x02;
const POP: u8 = 0x04;
const DUP: u8 = 0x05;
const SWAP: u8 = 0x06;

const ADD: u8 = 0x10;
const SUB: u8 = 0x11;
const MUL: u8 = 0x12;
const DIV: u8 = 0x13;
const MOD: u8 = 0x14;

const CMP: u8 = 0x20;
const EQ: u8 = 0x21;
const NEQ: u8 = 0x22;
const GT: u8 = 0x23;
const LT: u8 = 0x24;

const JMP32: u8 = 0x30;
const JZ32: u8 = 0x31;
const JNZ32: u8 = 0x32;
const CALL32: u8 = 0x33;
const RET: u8 = 0x34;

const LOAD: u8 = 0x40;
const STORE: u8 = 0x41;
const LOAD_ABS: u8 = 0x44;
const STORE_ABS: u8 = 0x45;

const SYSCALL: u8 = 0x50;
const BREAK: u8 = 0x51;


const SYSCALL_EXIT: u8 = 0x00;
const SYSCALL_READ: u8 = 0x02;
const SYSCALL_WRITE: u8 = 0x03;

pub struct NVMCodeGen {
    bytecode: Vec<u8>,
    labels: HashMap<String, u32>,
    label_patches: Vec<(u32, String)>,
    local_vars: HashMap<String, u8>,
    next_local: u8,
    loop_stack: Vec<(String, String)>,
    current_function: String,
}

impl NVMCodeGen {
    pub fn new() -> Self {
        Self {
            bytecode: Vec::new(),
            labels: HashMap::new(),
            label_patches: Vec::new(),
            local_vars: HashMap::new(),
            next_local: 0,
            loop_stack: Vec::new(),
            current_function: String::new(),
        }
    }

    pub fn generate(&mut self, program: &Program) -> Vec<u8> {
        
        self.bytecode.extend_from_slice(&[b'N', b'V', b'M', b'0']);

        
        if let Some(main_func) = program.functions.iter().find(|f| f.name == "main") {
            self.generate_function(main_func, program);
        }

        
        for func in &program.functions {
            if func.name != "main" {
                self.generate_function(func, program);
            }
        }

        
        for (_module_name, module) in &program.modules {
            for func in &module.functions {
                if func.is_exported {
                    let full_name = format!("{}_{}", module.name, func.name);
                    self.generate_module_function(func, &full_name, program);
                }
            }
        }

        
        self.patch_labels();

        self.bytecode.clone()
    }

    fn generate_function(&mut self, func: &Function, program: &Program) {
        self.current_function = func.name.clone();
        self.local_vars.clear();
        self.next_local = 0;

        
        let func_label = format!("func_{}", func.name);
        self.add_label(&func_label);

        
        for param in &func.params {
            self.local_vars.insert(param.name.clone(), self.next_local);
            self.next_local += 1;
        }

        
        for stmt in &func.body {
            self.generate_statement(stmt, program);
        }

        
        if func.name == "main" {
            
            self.emit_push32(0);
            self.emit_byte(SYSCALL);
            self.emit_byte(SYSCALL_EXIT);
        }
        
        self.emit_byte(RET);
    }

    fn generate_module_function(&mut self, func: &Function, full_name: &str, program: &Program) {
        self.current_function = full_name.to_string();
        self.local_vars.clear();
        self.next_local = 0;

        let func_label = format!("func_{}", full_name);
        self.add_label(&func_label);

        for param in &func.params {
            self.local_vars.insert(param.name.clone(), self.next_local);
            self.next_local += 1;
        }

        for stmt in &func.body {
            self.generate_statement(stmt, program);
        }

        self.emit_byte(RET);
    }

    fn generate_statement(&mut self, stmt: &Statement, program: &Program) {
        match stmt {
            Statement::VarDecl { name, var_type: _, value } => {
                if let Some(init_expr) = value {
                    self.generate_expression(init_expr, program);
                } else {
                    self.emit_push32(0);
                }
                
                let local_index = self.next_local;
                self.local_vars.insert(name.clone(), local_index);
                self.next_local += 1;
                
                self.emit_byte(STORE);
                self.emit_byte(local_index);
            }

            Statement::Assignment { name, value } => {
                self.generate_expression(value, program);
                
                if let Some(&local_index) = self.local_vars.get(name) {
                    self.emit_byte(STORE);
                    self.emit_byte(local_index);
                } else {
                    
                    panic!("Variable not found: {}", name);
                }
            }

            Statement::If { condition, then_body, else_body } => {
                self.generate_expression(condition, program);
                
                let else_label = self.generate_label("else");
                let end_label = self.generate_label("endif");
                
                self.emit_byte(JZ32);
                self.emit_label_ref(&else_label);
                
                for stmt in then_body {
                    self.generate_statement(stmt, program);
                }
                
                self.emit_byte(JMP32);
                self.emit_label_ref(&end_label);
                
                self.add_label(&else_label);
                
                if let Some(else_stmts) = else_body {
                    for stmt in else_stmts {
                        self.generate_statement(stmt, program);
                    }
                }
                
                self.add_label(&end_label);
            }

            Statement::For { init, condition, post, body } => {
                
                if let Some(init_stmt) = init {
                    self.generate_statement(init_stmt, program);
                }
                
                let loop_start = self.generate_label("for_start");
                let loop_end = self.generate_label("for_end");
                let loop_continue = self.generate_label("for_continue");
                
                self.loop_stack.push((loop_end.clone(), loop_continue.clone()));
                
                self.add_label(&loop_start);
                
                
                if let Some(cond) = condition {
                    self.generate_expression(cond, program);
                    self.emit_byte(JZ32);
                    self.emit_label_ref(&loop_end);
                }
                
                
                for stmt in body {
                    self.generate_statement(stmt, program);
                }
                
                self.add_label(&loop_continue);
                
                
                if let Some(post_stmt) = post {
                    self.generate_statement(post_stmt, program);
                }
                
                self.emit_byte(JMP32);
                self.emit_label_ref(&loop_start);
                
                self.add_label(&loop_end);
                self.loop_stack.pop();
            }

            Statement::Return(value) => {
                if let Some(expr) = value {
                    self.generate_expression(expr, program);
                }
                self.emit_byte(RET);
            }

            Statement::Expression(expr) => {
                self.generate_expression(expr, program);
                self.emit_byte(POP);
            }

            _ => {}
        }
    }

    fn generate_expression(&mut self, expr: &Expression, program: &Program) {
        match expr {
            Expression::Number(n) => {
                self.emit_push32(*n as i32);
            }

            Expression::String(_s) => {
                self.emit_push32(0);
            }

            Expression::Identifier(name) => {
                if let Some(&local_index) = self.local_vars.get(name) {
                    self.emit_byte(LOAD);
                    self.emit_byte(local_index);
                } else {
                    panic!("Variable not found: {}", name);
                }
            }

            Expression::Binary { op, left, right } => {
                self.generate_expression(left, program);
                self.generate_expression(right, program);
                
                match op {
                    BinaryOp::Add => self.emit_byte(ADD),
                    BinaryOp::Sub => self.emit_byte(SUB),
                    BinaryOp::Mul => self.emit_byte(MUL),
                    BinaryOp::Div => self.emit_byte(DIV),
                    BinaryOp::Mod => self.emit_byte(MOD),
                    BinaryOp::Equal => self.emit_byte(EQ),
                    BinaryOp::NotEqual => self.emit_byte(NEQ),
                    BinaryOp::Less => self.emit_byte(LT),
                    BinaryOp::Greater => self.emit_byte(GT),
                    BinaryOp::LessEqual => {
                        // a <= b is !(a > b)
                        self.emit_byte(GT);
                        self.emit_push32(0);
                        self.emit_byte(EQ);
                    }
                    BinaryOp::GreaterEqual => {
                        // a >= b is !(a < b)
                        self.emit_byte(LT);
                        self.emit_push32(0);
                        self.emit_byte(EQ);
                    }
                    _ => {}
                }
            }

            Expression::Unary { op, operand } => {
                self.generate_expression(operand, program);
                
                match op {
                    UnaryOp::Neg => {
                        // Negate: 0 - x
                        self.emit_push32(0);
                        self.emit_byte(SWAP);
                        self.emit_byte(SUB);
                    }
                    UnaryOp::Not => {
                        // Logical not: x == 0
                        self.emit_push32(0);
                        self.emit_byte(EQ);
                    }
                }
            }

            Expression::Call { function, args } => {
                // Push arguments in reverse order (right-to-left)
                for arg in args.iter().rev() {
                    self.generate_expression(arg, program);
                }
                
                let func_label = format!("func_{}", function);
                self.emit_byte(CALL32);
                self.emit_label_ref(&func_label);
            }

            Expression::ModuleCall { module, function, args } => {
                // Special-case stdio PrintStr/PrintlnStr: write directly to VGA buffer for string literals
                if module == "stdio" && (function == "PrintStr" || function == "PrintlnStr") {
                    if !args.is_empty() {
                        if let Expression::String(s) = &args[0] {
                            // Print each character directly using store_abs to VGA
                            let base_addr: u32 = 0xB8F00; // last line start like HelloWorld
                            let attr: u32 = 0x07; // white on black
                            for (i, ch) in s.as_bytes().iter().enumerate() {
                                let addr = base_addr + (i as u32 * 2);
                                let val = ((attr as u32) << 8) | (*ch as u32);
                                self.emit_push32(addr as i32);
                                self.emit_push32(val as i32);
                                self.emit_byte(STORE_ABS);
                            }
                            return;
                        }
                    }
                    // Fallback: generate call to stdio implementation if not a string literal
                }

                // Push arguments in reverse order
                for arg in args.iter().rev() {
                    self.generate_expression(arg, program);
                }

                let func_label = format!("func_{}_{}", module, function);
                self.emit_byte(CALL32);
                self.emit_label_ref(&func_label);
            }

            _ => {
                // Unsupported expressions
                self.emit_push32(0);
            }
        }
    }

    fn emit_byte(&mut self, byte: u8) {
        self.bytecode.push(byte);
    }

    fn emit_push32(&mut self, value: i32) {
        self.emit_byte(PUSH32);
        let bytes = value.to_be_bytes();
        self.bytecode.extend_from_slice(&bytes);
    }

    fn emit_label_ref(&mut self, label: &str) {
        let pos = self.bytecode.len() as u32;
        self.label_patches.push((pos, label.to_string()));
        // Emit placeholder
        self.bytecode.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn add_label(&mut self, label: &str) {
        let pos = self.bytecode.len() as u32;
        self.labels.insert(label.to_string(), pos);
    }

    fn generate_label(&self, prefix: &str) -> String {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}_{}_{}", prefix, self.current_function, count)
    }

    fn patch_labels(&mut self) {
        for (pos, label) in &self.label_patches {
            if let Some(&target) = self.labels.get(label) {
                let bytes = target.to_be_bytes();
                let pos = *pos as usize;
                self.bytecode[pos..pos + 4].copy_from_slice(&bytes);
            } else {
                eprintln!("Warning: Unresolved label: {}", label);
            }
        }
    }
}
