use std::mem;
use std::str;
use std::ops::Range;
use std::collections::HashMap;
use string_interner::Symbol as _;
use crate::language::{IntType, FloatType};
use crate::runtime::{Variant, STRING_TABLE};
use crate::runtime::strings::{StringInterner, InternSymbol, StringSymbol};
use crate::runtime::types::function::Signature;
use crate::codegen::errors::{CompileResult, CompileError, ErrorKind};
use crate::debug::DebugSymbol;


// these are limited to u16 right now because they are loaded by opcodes
pub type ConstID = u16;
pub type ChunkID = u16;
pub type FunctionID = u16;


// Constants

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constant {
    Integer(IntType),
    Float([u8; mem::size_of::<FloatType>()]),
    String(usize),
    Function(ChunkID, FunctionID),
}

impl From<IntType> for Constant {
    fn from(value: IntType) -> Self { Self::Integer(value) }
}

impl From<FloatType> for Constant {
    fn from(value: FloatType) -> Self { Self::Float(value.to_le_bytes()) }
}

impl From<usize> for Constant {
    fn from(index: usize) -> Self { Self::String(index) }
}

impl From<InternSymbol> for Constant {
    fn from(symbol: InternSymbol) -> Self { Self::String(symbol.to_usize()) }
}


/// A buffer used by ChunkBuilder
#[derive(Default)]
pub struct ChunkBuf {
    bytes: Vec<u8>,
    symbol: Option<DebugSymbol>,
}

impl ChunkBuf {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            symbol: None,
        }
    }
    
    // Bytes
    
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    
    pub fn as_slice(&self) -> &[u8] {
        self.bytes.as_slice()
    }
    
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.bytes.as_mut_slice()
    }
    
    // using Into<u8> so that OpCodes can be accepted without extra fuss
    pub fn push_byte(&mut self, byte: impl Into<u8>) {
        self.bytes.push(byte.into());
    }
    
    pub fn extend_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
    }
    
    pub fn patch_bytes(&mut self, offset: usize, patch: &[u8]) {
        let patch_range = offset..(offset + patch.len());
        let target = &mut self.bytes[patch_range];
        target.copy_from_slice(patch);
    }
    
    /// anything previously inside the patch is overwritten
    pub fn resize_patch(&mut self, offset: usize, from_len: usize, to_len: usize) {
        let patch_range = offset..(offset + from_len);
        let patch = std::iter::repeat(u8::default()).take(to_len);
        self.bytes.splice(patch_range, patch);
    }
}


pub struct ChunkBuilder {
    chunks: Vec<ChunkBuf>,
    consts: Vec<Constant>,
    functions: Vec<Signature>,
    dedup: HashMap<Constant, ConstID>,
    strings: StringInterner,
}

impl ChunkBuilder {
    pub fn new() -> Self {
        Self {
            consts: Vec::new(),
            functions: Vec::new(),
            chunks: vec![ ChunkBuf::new() ],
            dedup: HashMap::new(),
            strings: StringInterner::new(),
        }
    }
    
    pub fn with_strings(strings: StringInterner) -> Self {
        Self {
            chunks: vec![ ChunkBuf::new() ],
            functions: Vec::new(),
            consts: Vec::new(),
            dedup: HashMap::new(),
            strings,
        }
    }
    
    // Bytecode
    
    pub fn new_chunk(&mut self) -> CompileResult<ChunkID> {
        let chunk_id = ChunkID::try_from(self.chunks.len())
            .map_err(|_| CompileError::from(ErrorKind::ChunkCountLimit))?;
        
        self.chunks.push(ChunkBuf::new());
        Ok(chunk_id)
    }

    pub fn chunk(&self, chunk_id: ChunkID) -> &ChunkBuf { 
        &self.chunks[usize::from(chunk_id)]
    }
    
    pub fn chunk_mut(&mut self, chunk_id: ChunkID) -> &mut ChunkBuf { 
        &mut self.chunks[usize::from(chunk_id)]
    }
    
    // Constants
    
    pub fn get_or_insert_const(&mut self, value: Constant) -> CompileResult<ConstID> {
        if let Constant::String(index) = value {
            let symbol = InternSymbol::try_from_usize(index);
            debug_assert!(self.strings.resolve(symbol.unwrap()).is_some());
        }
        
        if let Some(cid) = self.dedup.get(&value) {
            Ok(*cid)
        } else {
            let cid = ConstID::try_from(self.consts.len())
                .map_err(|_| CompileError::from(ErrorKind::ConstPoolLimit))?;
            self.consts.push(value);
            self.dedup.insert(value, cid);
            Ok(cid)
        }
    }
    
    pub fn get_or_insert_str(&mut self, string: &str) -> CompileResult<ConstID> {
        let symbol = self.strings.get_or_intern(string);
        self.get_or_insert_const(Constant::String(symbol.to_usize()))
    }
    
    pub fn build(self) -> UnloadedProgram {
        let mut bytes = Vec::new();
        let mut chunks = Vec::with_capacity(self.chunks.len());
        
        for chunk in self.chunks.into_iter() {
            let offset = bytes.len();
            let length = chunk.bytes.len();
            bytes.extend(chunk.bytes);
            
            let index = ChunkIndex {
                offset, length,
                symbol: chunk.symbol,
            };
            chunks.push(index);
        }
        
        let mut strings = Vec::new();
        strings.resize_with(self.strings.len(), StringIndex::default);
        
        for (symbol, string) in self.strings.into_iter() {
            let string = string.as_bytes();
            let offset = bytes.len();
            let length = string.len();
            bytes.extend(string);
            
            let index = StringIndex {
                offset, length
            };
            strings.insert(symbol.to_usize(), index);
        }
        
        UnloadedProgram {
            bytes: bytes.into_boxed_slice(),
            chunks: chunks.into_boxed_slice(),
            strings: strings.into_boxed_slice(),
            consts: self.consts.into_boxed_slice(),
            functions: self.functions.into_boxed_slice(),
        }
    }
}


// TODO store all chunk bytes in a single array
// TODO figure out how debug symbols will work, esp. at runtime

#[derive(Debug, Default, Clone)]
pub struct ChunkIndex {
    offset: usize,
    length: usize,
    symbol: Option<DebugSymbol>, // will be used for tracebacks
}

impl ChunkIndex {
    pub fn as_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.length)
    }
    
    pub fn debug_symbols(&self) -> Option<&DebugSymbol> {
        self.symbol.as_ref()
    }
}

#[derive(Debug, Default, Clone)]
pub struct StringIndex {
    offset: usize,
    length: usize,
}

impl StringIndex {
    pub fn as_range(&self) -> Range<usize> {
        self.offset..(self.offset + self.length)
    }
}

/// A program whose strings have not been yet been loaded into the thread-local string table
/// This means that an `UnloadedProgram` cannot be executed. However, it also means that an
/// `UnloadedProgram` is also self-contained, which is useful for exporting to a file or
/// between threads.
#[derive(Debug, Clone)]
pub struct UnloadedProgram {
    bytes: Box<[u8]>,
    chunks: Box<[ChunkIndex]>,
    strings: Box<[StringIndex]>,
    consts: Box<[Constant]>,
    functions: Box<[Signature]>,
}

impl UnloadedProgram {
    pub fn chunk(&self, chunk_id: ChunkID) -> &[u8] {
        let chunk_idx = &self.chunks[usize::from(chunk_id)];
        &self.bytes[chunk_idx.as_range()]
    }
    
    pub fn chunk_ids(&self) -> impl Iterator<Item=ChunkID> {
        ChunkID::from(0u16)..ChunkID::try_from(self.chunks.len()).unwrap()
    }
    
    pub fn string(&self, index: usize) -> &str {
        let string_idx = &self.strings[index];
        str::from_utf8(&self.bytes[string_idx.as_range()]).expect("invalid string")
    }
    
    pub fn iter_strings(&self) -> impl Iterator<Item=&str> {
        self.strings.iter()
            .map(|index| &self.bytes[index.as_range()])
            .map(|slice| str::from_utf8(slice).expect("invalid string"))
    }
    
    pub fn lookup_const(&self, index: impl Into<ConstID>) -> &Constant {
        &self.consts[usize::from(index.into())]
    }
}


/// Unlike `UnloadedProgram`, this is not `Send` (mainly because `StringSymbol` is not Send)

#[derive(Debug)]
pub struct Program {
    bytes: Box<[u8]>,
    chunks: Box<[ChunkIndex]>,
    consts: Box<[Constant]>,
    functions: Box<[Signature]>,
    strings: Box<[StringSymbol]>,
}

impl Program {
    #[inline(always)]
    pub fn chunk(&self, chunk_id: ChunkID) -> &[u8] {
        let chunk = &self.chunks[usize::from(chunk_id)];
        &self.bytes[chunk.as_range()]
    }
    
    pub fn lookup_const(&self, index: impl Into<ConstID>) -> &Constant {
        &self.consts[usize::from(index.into())]
    }
    
    pub fn lookup_value(&self, index: impl Into<ConstID>) -> Variant {
        match self.lookup_const(index) {
            Constant::Integer(value) => Variant::from(*value),
            Constant::Float(bytes) => FloatType::from_le_bytes(*bytes).into(),
            Constant::String(idx) => Variant::from(self.strings[idx.to_usize()]),
            Constant::Function(chunk_id, function_id) => unimplemented!(), // get or create function object
        }
    }
    
    pub fn load(program: UnloadedProgram) -> Self {
        let strings = STRING_TABLE.with(|string_table| {
            let mut interner = string_table.interner_mut();
            
            let mut strings = Vec::with_capacity(program.strings.len());
            for string in program.iter_strings() {
                let symbol = StringSymbol::from(interner.get_or_intern(string));
                strings.push(symbol);
            }
            
            strings.into_boxed_slice()
        });
        
        let byte_len = program.chunk_ids()
            .map(|chunk_id| program.chunk(chunk_id).len())
            .sum();
        
        let mut bytes = program.bytes.into_vec();
        bytes.truncate(byte_len);
        
        Self {
            bytes: bytes.into_boxed_slice(),
            chunks: program.chunks,
            consts: program.consts,
            functions: program.functions,
            strings
        }
    }
    
    /// prepares an `UnloadedProgram` for exporting to a file
    pub fn unload(self) -> UnloadedProgram {
        let mut bytes = self.bytes.into_vec();
        let mut strings = Vec::with_capacity(self.strings.len());
        
        STRING_TABLE.with(|string_table| {
            let interner = string_table.interner();
            
            for symbol in self.strings.into_iter() {
                let string = interner
                    .resolve((*symbol).into())
                    .unwrap().as_bytes();
                
                let offset = bytes.len();
                let length = string.len();
                let index = StringIndex {
                    offset, length,
                };
                
                bytes.extend(string);
                strings.push(index);
            }
            
        });
        
        UnloadedProgram {
            bytes: bytes.into_boxed_slice(),
            chunks: self.chunks,
            strings: strings.into_boxed_slice(),
            consts: self.consts,
            functions: self.functions,
        }
    }
}
