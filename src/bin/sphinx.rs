use std::io::{self, Write};
use std::path::PathBuf;
use clap::{Command, Arg, ArgMatches};

use sphinx_lang;
use sphinx_lang::frontend;
use sphinx_lang::BuildErrors;
use sphinx_lang::source::{ModuleSource, SourceType, SourceText};
use sphinx_lang::parser::stmt::{Stmt, StmtMeta};
use sphinx_lang::codegen::{Program, CompiledProgram};
use sphinx_lang::runtime::VirtualMachine;
use sphinx_lang::runtime::module::{ModuleCache, GlobalEnv};
use sphinx_lang::runtime::strings::StringInterner;
use sphinx_lang::debug::symbol::BufferedResolver;

fn main() {
    env_logger::init();
    
    let app = Command::new("sphinx")
        .version("0.0")
        .author("M. Werezak <mwerezak@gmail.com>")
        .about("An interpreter for the Sphinx programming language")
        .arg(
            Arg::new("file")
            .index(1)
            .help("Path to input script file")
            .value_name("FILE")
        )
        .arg(
            Arg::new("cmd")
            .short('c')
            .help("Execute a snippet then exit")
            .value_name("CMD")
        )
        .arg(
            Arg::new("interactive")
            .short('i')
            .help("Drop into an interactive REPL after executing")
        )
        .arg(
            Arg::new("parse_only")
            .short('P')
            .help("Parse and print AST instead of executing")
        )
        .arg(
            Arg::new("compile_only")
            .short('d')
            .help("Produce compiled bytecode instead of executing (not implemented)")
        );
    
    let version = app.get_version().unwrap();
    let args = app.get_matches();
    
    let mut module = None;
    if let Some(s) = args.value_of("cmd") {
        let source = SourceType::String(s.to_string());
        module = Some(ModuleSource::new("<cmd>", source));
    } else if let Some(s) = args.value_of("file") {
        let source = SourceType::File(PathBuf::from(s));
        module = Some(ModuleSource::new(s, source));
    }
    
    if module.is_none() {
        let mut module_cache = ModuleCache::new();
        let repl_env = GlobalEnv::new();
        
        println!("\nSphinx Version {}\n", version);
        Repl::new(&mut module_cache, &repl_env).run();
        return;
    }
    
    let module = module.unwrap();
    
    if args.is_present("parse_only") {
        parse_and_print_ast(&args, module);
    }
    else if args.is_present("compile_only") {
        unimplemented!()
    }
    else if args.is_present("interactive") {
        if let Some(build) = build_program(&args, &module) {
            let program = Program::load(build.program);
            
            let mut module_cache = ModuleCache::new();
            let module_id = module_cache.insert(module, program.data);
            
            let repl_env = GlobalEnv::new();
            
            let mut vm = VirtualMachine::new_repl(&module_cache, &repl_env, module_id, &program.main);
            vm.run().expect("runtime error");
            
            println!("\nSphinx Version {}\n", version);
            Repl::new(&mut module_cache, &repl_env).run()
        }
    }
    else {
        if let Some(build) = build_program(&args, &module) {
            let program = Program::load(build.program);
            
            let mut module_cache = ModuleCache::new();
            let module_id = module_cache.insert(module, program.data);
            
            let mut vm = VirtualMachine::new(&module_cache, module_id, &program.main);
            vm.run().expect("runtime error");
        }
    }
}


fn build_program<'m>(_args: &ArgMatches, source: &ModuleSource) -> Option<CompiledProgram> {
    // build module
    let build_result = sphinx_lang::build_module(source);
    if build_result.is_err() {
        match build_result.unwrap_err() {
            BuildErrors::Source(error) => {
                println!("Error reading source: {}.", error);
            }
            
            BuildErrors::Syntax(errors) => {
                println!("Errors in file \"{}\":\n", source.name());
                frontend::print_source_errors(source, &errors);
            }
            
            BuildErrors::Compile(errors) => {
                println!("Errors in file \"{}\":\n", source.name());
                frontend::print_source_errors(source, &errors);
            }
        }
        return None;
    }
    
    return Some(build_result.unwrap())
}


fn parse_and_print_ast(_args: &ArgMatches, module: ModuleSource) {
    let source_text = match module.source_text() {
        Ok(source_text) => source_text,
        
        Err(error) => {
            println!("Error reading source: {}.", error);
            return;
        },
    };
    
    let mut interner = StringInterner::new();
    let parse_result = sphinx_lang::parse_source(&mut interner, source_text);
    
    match parse_result {
        Err(errors) => {
            println!("Errors in file \"{}\":\n", module.name());
            frontend::print_source_errors(&module, &errors);
        },
        Ok(ast) => println!("{:#?}", ast),
    }
}


//////// REPL ////////


const PROMT_START: &str = ">>> ";
const PROMT_CONTINUE: &str = "... ";

struct Repl<'m> {
    module_cache: &'m mut ModuleCache,
    repl_env: &'m GlobalEnv,
}

enum ReadLine {
    Ok(String),
    Empty,
    Restart,
    Quit,
}

impl<'m> Repl<'m> {
    pub fn new(module_cache: &'m mut ModuleCache, repl_env: &'m GlobalEnv) -> Self {
        Self {
            module_cache, repl_env,
        }
    }
    
    fn read_line(&self, prompt: &'static str) -> ReadLine {
        io::stdout().write(prompt.as_bytes()).unwrap();
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        let result = io::stdin().read_line(&mut input);
        if result.is_err() {
            println!("Could not read input: {}", result.unwrap_err());
            return ReadLine::Restart;
        }
        
        input = input.trim_end().to_string();
        
        if input.is_empty() {
            return ReadLine::Empty;
        }
        
        if input == "quit" || input.chars().find(|c| *c == '\x04').is_some() {
            return ReadLine::Quit;
        }
        
        ReadLine::Ok(input)
    }
    
    pub fn run(&mut self) {
        
        loop {
            let mut interner;
            let mut input = String::new();
            let mut parse_result = None;
            
            loop {
                let prompt =
                    if input.is_empty() { PROMT_START }
                    else { PROMT_CONTINUE };
                
                interner = StringInterner::new();
                
                match self.read_line(prompt) {
                    ReadLine::Quit => return,
                    ReadLine::Restart => continue,
                    ReadLine::Empty => {
                        if input.is_empty() { continue }
                        else { break }
                    },
                    ReadLine::Ok(line) => {
                        input.push_str(&line);
                        
                        if line.trim_end().ends_with(';') {
                            break
                        }
                        
                        // If we can't parse the input without errors, then we assume we need to continue
                        let source_text = SourceText::from(input.clone());
                        if let Ok(ast) = sphinx_lang::parse_source(&mut interner, source_text) {
                            parse_result.replace(ast);
                            break
                        }
                        
                        input.push('\n')
                    }
                }
            }
            
            let parse_result =
                if let Some(ast) = parse_result { Ok(ast) }
                else { 
                    let source_text = SourceText::from(input.clone());
                    sphinx_lang::parse_source(&mut interner, source_text) 
                };
            
            let mut ast = match parse_result {
                Ok(ast) => ast,
                
                Err(errors) => {
                    let resolver = BufferedResolver::new(input);
                    frontend::print_source_errors(&resolver, &errors);
                    continue;
                },
            };
            
            // if the last stmt is an expression statement, convert it into an inspect
            if let Some(stmt) = ast.pop() {
                let (mut stmt, symbol) = stmt.take();
                if let Stmt::Expression(expr) = stmt {
                    stmt = Stmt::Echo(expr);
                }
                ast.push(StmtMeta::new(stmt, symbol))
            }
            
            let build = match sphinx_lang::compile_ast(interner, ast) {
                Ok(build) => build,
                
                Err(errors) => {
                    let resolver = BufferedResolver::new(input);
                    frontend::print_source_errors(&resolver, &errors);
                    continue;
                }
            };
            
            let program = Program::load(build.program);
            
            let module_source = ModuleSource::new("<repl>", SourceType::String(input));
            let module_id = self.module_cache.insert(module_source, program.data);
            
            let mut vm = VirtualMachine::new_repl(self.module_cache, self.repl_env, module_id, &program.main);
            if let Err(error) = vm.run() {
                println!("Runtime error: {:?}", error);
            }
            
        }
        
    }
}
