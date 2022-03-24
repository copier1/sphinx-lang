use std::cmp::Ordering;
use std::collections::HashMap;
use crate::codegen::ChunkID;
use crate::debug::symbol::DebugSymbol;

pub type ChunkSymbols = HashMap<Option<ChunkID>, DebugSymbolTable>;

/// Maps bytecode offsets to DebugSymbols
#[derive(Debug)]
pub struct DebugSymbolTable {
    entries: Vec<SymbolTableEntry>,
}

impl Default for DebugSymbolTable {
    fn default() -> Self { Self::new() }
}

impl DebugSymbolTable {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }
    
    pub fn insert(&mut self, offset: usize, symbol: Option<DebugSymbol>) {
        let entry = SymbolTableEntry(offset, symbol);
        
        if matches!(self.entries.last(), Some(last_entry) if entry <= *last_entry) {
            panic!("symbol inserted out of order");
        }
        
        self.entries.push(entry)
    }
    
    pub fn lookup(&self, offset: usize) -> Option<&DebugSymbol> {
        if let Ok(index) = self.entries.binary_search_by_key(&offset, |entry| entry.0) {
            self.entries[index].1.as_ref()
        } else {
            None
        }
    }
    
    pub fn iter(&self) -> impl Iterator<Item=(usize, Option<&DebugSymbol>)> + '_ {
        self.entries.iter().map(|entry| {
            let SymbolTableEntry(offset, symbol) = entry;
            (*offset, symbol.as_ref())
        })
    }
    
    pub fn symbols(&self) -> impl Iterator<Item=&DebugSymbol> {
        self.entries.iter().filter_map(|entry| {
            let SymbolTableEntry(_, symbol) = entry;
            symbol.as_ref()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SymbolTableEntry(usize, Option<DebugSymbol>);

impl PartialOrd for SymbolTableEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        usize::partial_cmp(&self.0, &other.0)
    }
}

impl Ord for SymbolTableEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        usize::cmp(&self.0, &other.0)
    }
}
