#![cfg(test)]

use crate::source::{ModuleSource, SourceText, SourceType};
use super::symbol::{DebugSymbol, ResolvedSymbol, DebugSymbolResolver};

#[test]
fn debug_symbols_test_symbol_resolution() {
    let text = r#"example
        code this example
        another example"#;
    
    let module = ModuleSource::new("<test>", SourceType::String(text.to_string()));
    
    let symbols = vec![
        DebugSymbol::from((0, 7)),
        DebugSymbol::from((0, 20)),
        DebugSymbol::from((13, 24)),
    ];
    
    let symbol_table = module.resolve_symbols(symbols.into_iter()).unwrap();
    
    for (k, v) in symbol_table.iter() {
        match v {
            Ok(symbol) => println!("{:?} => {}", k, symbol.as_single_line_fmt()),
            _ => println!("{:?} => {:?}", k, v),
        }
    }
}


use crate::debug::dasm::Disassembler;
use crate::runtime::Variant;
use crate::runtime::bytecode::{OpCode, Chunk};


#[test]
fn dasm_test_opcode_const() {
    let mut chunk = Chunk::new();
    
    let id = chunk.push_const(Variant::Float(1.2));
    chunk.push_byte(OpCode::LoadConst);
    chunk.push_byte(id as u8);
    
    let id = chunk.push_const(Variant::Boolean(true));
    chunk.push_byte(OpCode::LoadConstWide);
    chunk.extend_bytes(&id.to_le_bytes());
    
    chunk.push_byte(OpCode::Return);
    
    let dasm = Disassembler::new(&chunk);
    println!("== dasm_test_opcode_const ==");
    println!("{}", dasm);
}