mod lexer;
mod parser;
mod ast;
mod elf;
mod pe;
mod nvm;
mod error;

use std::fs;
use std::env;
use std::process;
use std::collections::HashSet;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <source.per> [--elf|--nvm-code|--novaria]", args[0]);
        process::exit(1);
    }

    let source_file = &args[1];
    let source = match fs::read_to_string(source_file) {
        Ok(s) => s,
        Err(e) => {
            let err = error::CompileError::new(
                error::ErrorKind::ModuleError,
                format!("failed to read source file: {}", e),
                source_file.to_string(),
                1,
                1,
            );
            err.display();
            process::exit(1);
        }
    };

    let mut lexer = lexer::Lexer::new_with_file(&source, source_file);
    let tokens = lexer.tokenize();

    let mut parser = parser::Parser::new(tokens, source_file);
    let mut ast = match parser.parse() {
        Ok(ast) => ast,
        Err(e) => {
            e.display();
            process::exit(1);
        }
    };

    let source_dir = std::path::Path::new(source_file).parent().unwrap_or(std::path::Path::new("."));
    if let Err(e) = load_modules(&mut ast, source_dir, &mut std::collections::HashSet::new()) {
        e.display();
        process::exit(1);
    }

    let target = if args.len() > 2 {
        match args[2].as_str() {
            "--elf" => "elf",
            "--nvm-code" => "nvm-code",
            "--novaria" => "novaria",
            _ => {
                eprintln!("Unknown target: {}", args[2]);
                eprintln!("Valid targets: --elf, --nvm-code, --novaria");
                process::exit(1);
            }
        }
    } else if cfg!(target_os = "windows") {
        "pe"
    } else {
        "elf"
    };

    let output_file = match target {
        "nvm-code" => {
            if source_file.ends_with(".per") {
                source_file.replace(".per", ".asm")
            } else {
                format!("{}.asm", source_file)
            }
        }
        "novaria" => {
            if source_file.ends_with(".per") {
                source_file.replace(".per", ".bin")
            } else {
                format!("{}.bin", source_file)
            }
        }
        "elf" => {
            if source_file.ends_with(".per") {
                source_file[..source_file.len()-4].to_string()
            } else if source_file.ends_with(".nl") {
                source_file[..source_file.len()-4].to_string()
            } else {
                source_file.to_string()
            }
        }
        _ => {
            if source_file.ends_with(".per") {
                source_file.replace(".per", ".exe")
            } else {
                source_file.replace(".go", ".exe")
            }
        }
    };

    match target {
        "novaria" => {
            compile_nvm(&ast, &output_file);
        }
        "nvm-code" => {
            compile_nvm_asm(&ast, &output_file);
        }
        "elf" => {
            compile_elf_proper(&ast, &output_file);
        }
        _ => {
            let mut codegen = pe::CodeGen::new(target);
            let machine_code = codegen.generate(&ast);
            let mut pe_writer = pe::PEWriter::new();
            pe_writer.write(&output_file, &machine_code)
                .expect("Failed to write executable");
        }
    }

    println!("Compilation successful: {}", output_file);
}

fn load_modules(ast: &mut ast::Program, base_dir: &Path, loaded: &mut HashSet<String>) -> error::Result<()> {
    let imports = ast.imports.clone();

    for import in &imports {
        let module_name = import.path.clone();

        if loaded.contains(&module_name) {
            continue;
        }

        loaded.insert(module_name.clone());

        
        let module_filename = format!("{}.per", module_name);
        
        
        let mut module_file = base_dir.join(&module_filename);
        
        
        if !module_file.exists() {
            module_file = Path::new("stdlib").join(&module_filename);
        }
        
        
        if !module_file.exists() {
            if let Ok(exe_path) = env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    module_file = exe_dir.join("stdlib").join(&module_filename);
                }
            }
        }

        let module_source = match fs::read_to_string(&module_file) {
            Ok(s) => s,
            Err(_) => {
                return Err(error::CompileError::new(
                    error::ErrorKind::ModuleError,
                    format!("could not find module '{}'", module_name),
                    module_file.to_string_lossy().to_string(),
                    1,
                    1,
                ).with_source_line(format!("import \"{}\"", module_name)));
            }
        };

        let mut module_lexer = lexer::Lexer::new_with_file(&module_source, &module_file.to_string_lossy());
        let module_tokens = module_lexer.tokenize();
        let mut module_parser = parser::Parser::new(module_tokens, &module_file.to_string_lossy());
        let mut module_ast = module_parser.parse()?;

        load_modules(&mut module_ast, base_dir, loaded)?;

        for (mod_name, module) in module_ast.modules {
            ast.modules.insert(mod_name, module);
        }

        let module = ast::Module {
            name: module_name.clone(),
            functions: module_ast.functions,
        };

        ast.modules.insert(module_name, module);
    }

    Ok(())
}

fn compile_nvm(ast: &ast::Program, output_file: &str) {
    use std::io::Write;

    let mut nvm_gen = nvm::NVMCodeGen::new();
    let bytecode = nvm_gen.generate(ast);

    let mut file = fs::File::create(output_file).expect("Failed to create .nvm file");
    file.write_all(&bytecode).expect("Failed to write NVM bytecode");
}

fn compile_nvm_asm(ast: &ast::Program, output_file: &str) {
    use std::io::Write;

    let mut nvm_asm_gen = nvm::NVMAssemblyGenerator::new();
    let asm_code = nvm_asm_gen.generate(ast);

    let mut file = fs::File::create(output_file).expect("Failed to create .asm file");
    file.write_all(asm_code.as_bytes()).expect("Failed to write NVM assembly");
}

fn compile_elf_proper(ast: &ast::Program, output_file: &str) {
    use std::io::Write;

    let mut asm_gen = elf::AsmGenerator::new();
    let asm_code = asm_gen.generate(ast);

    let asm_file = format!("{}.s", output_file);
    let mut file = fs::File::create(&asm_file).expect("Failed to create .s file");
    file.write_all(asm_code.as_bytes()).expect("Failed to write assembly");

    let status = process::Command::new("gcc")
        .arg("-o")
        .arg(output_file)
        .arg(&asm_file)
        .arg("-no-pie")
        .status();

    match status {
        Ok(s) if s.success() => {
            let _ = fs::remove_file(&asm_file);
            println!("Compilation successful: {}", output_file);
        }
        Ok(s) => {
            eprintln!("GCC failed with exit code: {:?}", s.code());
            eprintln!("Assembly file kept at: {}", asm_file);
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to run gcc: {}", e);
            eprintln!("Make sure gcc is installed");
            eprintln!("Assembly file kept at: {}", asm_file);
            process::exit(1);
        }
    }
}