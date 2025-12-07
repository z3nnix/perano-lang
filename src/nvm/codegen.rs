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


// NovariaOS Syscall Numbers
const SYSCALL_EXIT: u8 = 0x00;
const SYSCALL_EXEC: u8 = 0x01;
const SYSCALL_READ: u8 = 0x02;
const SYSCALL_WRITE: u8 = 0x03;
const SYSCALL_CREATE: u8 = 0x04;
const SYSCALL_DELETE: u8 = 0x05;
const SYSCALL_CAP_CHECK: u8 = 0x06;
const SYSCALL_CAP_SPAWN: u8 = 0x07;
const SYSCALL_MSG_SEND: u8 = 0x09;
const SYSCALL_MSG_RECEIVE: u8 = 0x0A;
const SYSCALL_PORT_IN_BYTE: u8 = 0x0B;
const SYSCALL_PORT_OUT_BYTE: u8 = 0x0C;
const SYSCALL_PRINT: u8 = 0x0D;

// VGA text mode buffer address
const VGA_BUFFER: u32 = 0xB8000;

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

        // Generate helper function for printing integers
        self.generate_print_int_helper();

        
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
                // Special-case stdio functions: use SYS_PRINT syscall
                if module == "stdio" {
                    match function.as_str() {
                        "PrintStr" | "PrintlnStr" => {
                            if !args.is_empty() {
                                if let Expression::String(s) = &args[0] {
                                    // Print each character using SYS_PRINT syscall
                                    for ch in s.as_bytes() {
                                        self.emit_push32(*ch as i32);
                                        self.emit_byte(SYSCALL);
                                        self.emit_byte(SYSCALL_PRINT);
                                    }
                                    
                                    // If println, add newline
                                    if function == "PrintlnStr" {
                                        self.emit_push32('\n' as i32);
                                        self.emit_byte(SYSCALL);
                                        self.emit_byte(SYSCALL_PRINT);
                                    }
                                    
                                    // Push dummy value since this is an expression that returns void
                                    self.emit_push32(0);
                                    return;
                                }
                            }
                        }
                        "Print" => {
                            // Print integer - convert to string and print each digit
                            if !args.is_empty() {
                                self.generate_expression(&args[0], program);
                                // Call print_int helper
                                self.emit_byte(CALL32);
                                self.emit_label_ref("__print_int");
                                // Push dummy value since this is an expression that returns void
                                self.emit_push32(0);
                                return;
                            }
                        }
                        "Println" => {
                            // Print integer with newline
                            if !args.is_empty() {
                                self.generate_expression(&args[0], program);
                                self.emit_byte(CALL32);
                                self.emit_label_ref("__print_int");
                                self.emit_push32('\n' as i32);
                                self.emit_byte(SYSCALL);
                                self.emit_byte(SYSCALL_PRINT);
                                // Push dummy value since this is an expression that returns void
                                self.emit_push32(0);
                                return;
                            }
                        }
                        "PrintChar" => {
                            // Print single character
                            if !args.is_empty() {
                                self.generate_expression(&args[0], program);
                                self.emit_byte(SYSCALL);
                                self.emit_byte(SYSCALL_PRINT);
                                // Push dummy value since this is an expression that returns void
                                self.emit_push32(0);
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                // NovariaOS syscall integration
                if module == "novaria" {
                    // Special handling for file operations with string literals
                    match function.as_str() {
                        "FileCreateStr" => {
                            // FileCreateStr(filename: string, content: string)
                            if args.len() >= 2 {
                                if let (Expression::String(filename), Expression::String(content)) = (&args[0], &args[1]) {
                                    // Allocate strings in data section at known addresses
                                    // For simplicity, we'll use a fixed memory location
                                    // This is a hack - proper implementation needs data section
                                    
                                    // Push size
                                    self.emit_push32(content.len() as i32);
                                    
                                    // Push content pointer (we'll write string inline)
                                    let content_label = self.generate_label("str_content");
                                    self.emit_push32(0); // Placeholder for address
                                    let content_patch_pos = self.bytecode.len() - 4;
                                    
                                    // Push filename pointer
                                    let filename_label = self.generate_label("str_filename");
                                    self.emit_push32(0); // Placeholder for address
                                    let filename_patch_pos = self.bytecode.len() - 4;
                                    
                                    // Call SYS_CREATE
                                    self.emit_byte(SYSCALL);
                                    self.emit_byte(SYSCALL_CREATE);
                                    
                                    // Jump over string data
                                    let skip_label = self.generate_label("skip_strings");
                                    self.emit_byte(JMP32);
                                    self.emit_label_ref(&skip_label);
                                    
                                    // Emit filename string
                                    let filename_pos = self.bytecode.len();
                                    for ch in filename.as_bytes() {
                                        self.emit_byte(*ch);
                                    }
                                    self.emit_byte(0); // Null terminator
                                    
                                    // Emit content string
                                    let content_pos = self.bytecode.len();
                                    for ch in content.as_bytes() {
                                        self.emit_byte(*ch);
                                    }
                                    self.emit_byte(0); // Null terminator
                                    
                                    // Patch addresses
                                    let filename_addr = (filename_pos + 0x100000) as i32; // Add base offset
                                    let content_addr = (content_pos + 0x100000) as i32;
                                    
                                    // TODO: This won't work correctly - need proper data section
                                    // For now, return error
                                    
                                    self.add_label(&skip_label);
                                    
                                    // Push dummy return value
                                    self.emit_push32(0);
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                    
                    // Push arguments in reverse order (syscalls expect right-to-left)
                    for arg in args.iter().rev() {
                        self.generate_expression(arg, program);
                    }

                    // Generate syscall based on function name
                    match function.as_str() {
                        "Exit" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_EXIT);
                        }
                        "Exec" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_EXEC);
                        }
                        "FileRead" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_READ);
                        }
                        "FileWrite" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_WRITE);
                        }
                        "FileCreate" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_CREATE);
                        }
                        "FileDelete" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_DELETE);
                        }
                        "CapCheck" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_CAP_CHECK);
                        }
                        "CapSpawn" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_CAP_SPAWN);
                        }
                        "MsgSend" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_MSG_SEND);
                        }
                        "MsgReceive" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_MSG_RECEIVE);
                        }
                        "PortInByte" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_PORT_IN_BYTE);
                        }
                        "PortOutByte" => {
                            self.emit_byte(SYSCALL);
                            self.emit_byte(SYSCALL_PORT_OUT_BYTE);
                        }
                        // Capability constants - return immediately
                        "CAP_FS_READ" => {
                            self.emit_push32(1);
                        }
                        "CAP_FS_WRITE" => {
                            self.emit_push32(2);
                        }
                        "CAP_FS_CREATE" => {
                            self.emit_push32(4);
                        }
                        "CAP_FS_DELETE" => {
                            self.emit_push32(8);
                        }
                        "CAP_DRV_ACCESS" => {
                            self.emit_push32(16);
                        }
                        "CAP_CAPS_MGMT" => {
                            self.emit_push32(32);
                        }
                        "CAP_ALL" => {
                            self.emit_push32(65535);
                        }
                        _ => {
                            // Unknown novaria function, try as regular call
                            let func_label = format!("func_{}_{}", module, function);
                            self.emit_byte(CALL32);
                            self.emit_label_ref(&func_label);
                        }
                    }
                    return;
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

    fn generate_print_int_helper(&mut self) {
        // Helper function to print an integer
        // Input: integer on stack
        // Output: prints the integer character by character
        self.add_label("__print_int");
        
        // Load the value from stack (it's already on top)
        // Check if negative
        self.emit_byte(DUP);
        self.emit_push32(0);
        self.emit_byte(LT);
        
        let not_negative_label = self.generate_label("not_negative");
        self.emit_byte(JZ32);
        self.emit_label_ref(&not_negative_label);
        
        // If negative, print '-' and negate the value
        self.emit_push32('-' as i32);
        self.emit_byte(SYSCALL);
        self.emit_byte(SYSCALL_PRINT);
        
        // Negate the value
        self.emit_push32(0);
        self.emit_byte(SWAP);
        self.emit_byte(SUB);
        
        self.add_label(&not_negative_label);
        
        // Convert number to digits and print
        // Using iterative approach with a buffer on stack
        
        // Special case: if value is 0, just print '0'
        self.emit_byte(DUP);
        self.emit_push32(0);
        self.emit_byte(EQ);
        
        let not_zero_label = self.generate_label("not_zero");
        self.emit_byte(JZ32);
        self.emit_label_ref(&not_zero_label);
        
        // Print '0' and return
        self.emit_byte(POP); // Remove the duplicate 0
        self.emit_push32('0' as i32);
        self.emit_byte(SYSCALL);
        self.emit_byte(SYSCALL_PRINT);
        self.emit_byte(RET);
        
        self.add_label(&not_zero_label);
        
        // Digit extraction loop
        // We'll build digits in reverse, then print them
        self.emit_push32(0); // Digit count
        
        let digit_loop_label = self.generate_label("digit_loop");
        let digit_loop_end_label = self.generate_label("digit_loop_end");
        
        self.add_label(&digit_loop_label);
        
        // Check if value > 0
        self.emit_byte(SWAP);
        self.emit_byte(DUP);
        self.emit_push32(0);
        self.emit_byte(GT);
        
        self.emit_byte(JZ32);
        self.emit_label_ref(&digit_loop_end_label);
        
        // Extract digit: value % 10
        self.emit_byte(DUP);
        self.emit_push32(10);
        self.emit_byte(MOD);
        
        // Convert to ASCII: digit + '0'
        self.emit_push32('0' as i32);
        self.emit_byte(ADD);
        
        // Swap to get: count, digit, value
        self.emit_byte(SWAP);
        self.emit_byte(SWAP);
        
        // Divide value by 10
        self.emit_push32(10);
        self.emit_byte(DIV);
        
        // Swap to restore: value, digit, count
        self.emit_byte(SWAP);
        self.emit_byte(SWAP);
        
        // Increment count
        self.emit_push32(1);
        self.emit_byte(ADD);
        
        self.emit_byte(JMP32);
        self.emit_label_ref(&digit_loop_label);
        
        self.add_label(&digit_loop_end_label);
        
        // Pop the value (should be 0 now)
        self.emit_byte(SWAP);
        self.emit_byte(POP);
        
        // Now print digits (they're on stack in correct order)
        let print_loop_label = self.generate_label("print_loop");
        let print_loop_end_label = self.generate_label("print_loop_end");
        
        self.add_label(&print_loop_label);
        
        // Check if count > 0
        self.emit_byte(DUP);
        self.emit_push32(0);
        self.emit_byte(GT);
        
        self.emit_byte(JZ32);
        self.emit_label_ref(&print_loop_end_label);
        
        // Decrement count
        self.emit_push32(1);
        self.emit_byte(SUB);
        
        // Swap to get digit
        self.emit_byte(SWAP);
        
        // Print digit
        self.emit_byte(SYSCALL);
        self.emit_byte(SYSCALL_PRINT);
        
        self.emit_byte(JMP32);
        self.emit_label_ref(&print_loop_label);
        
        self.add_label(&print_loop_end_label);
        
        // Clean up count
        self.emit_byte(POP);
        
        self.emit_byte(RET);
    }
}
