use std::fs;
use std::path::{PathBuf, Path};
use std::io;
use crate::utils::ReadChars;

use crate::lexer::LexerBuilder;
use crate::parser::{Parser, ParserError};
use crate::parser::stmt::StmtMeta;
use crate::runtime::strings::StringInterner;

type ReadFileChars = ReadChars<io::BufReader<fs::File>>;

#[derive(Debug, Hash)]
pub enum SourceType {
    String(String),
    File(PathBuf),
}

#[derive(Debug)]
pub enum SourceText {
    String(String),
    File(ReadFileChars),
}

impl<S> From<S> for SourceText where S: ToString {
    fn from(text: S) -> Self { SourceText::String(text.to_string()) }
}

#[derive(Debug, Hash)]
pub struct ModuleSource {
    name: String,
    source: SourceType,
}

impl ModuleSource {
    pub fn new(name: impl ToString, source: SourceType) -> Self {
        ModuleSource {
            name: name.to_string(), 
            source,
        }
    }
    
    pub fn name(&self) -> &str { self.name.as_str() }
    pub fn source(&self) -> &SourceType { &self.source }
    
    // Load the source text
    pub fn source_text(&self) -> io::Result<SourceText> {
        match &self.source {
            SourceType::String(string) => Ok(SourceText::String(string.clone())),
            SourceType::File(ref path) => Ok(SourceText::File(Self::read_source_file(path)?)),
        }
    }
    
    fn read_source_file(path: &Path) -> io::Result<ReadFileChars> {
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        Ok(ReadChars::new(reader))
    }
    
}


/// High-level Parsing Interface
///
/// Contains the state required for parsing, and deals with the separate code paths taken for different SourceTypes
pub struct ParseContext<'f, 's> {
    lexer_factory: &'f LexerBuilder,
    interner: &'s mut StringInterner,
}

impl<'f, 's> ParseContext<'f, 's> {
    pub fn new(lexer_factory: &'f LexerBuilder, interner: &'s mut StringInterner) -> Self {
        ParseContext {
            lexer_factory,
            interner,
        }
    }
    
    // Returns a Vec of parsed Stmts (if no error occurred) or a Vec or errors
    pub fn parse_ast<'m>(&mut self, source: SourceText) -> Result<Vec<StmtMeta>, Vec<ParserError>> {
        
        let output = self.collect_parser_output(source);
        
        if output.iter().any(|r| r.is_err()) {
            Err(output.into_iter().filter_map(|r| r.err()).collect())
        } else {
            Ok(output.into_iter().filter_map(|r| r.ok()).collect())
        }
    }

    // Helper to deal with the separate branches for parsing SourceText
    fn collect_parser_output<'m>(&mut self, source: SourceText) -> Vec<Result<StmtMeta, ParserError>> {
        match source {
            SourceText::String(text) => {
                let mut chars = Vec::with_capacity(text.len());
                chars.extend(text.chars().map(Ok));
                
                let lexer = self.lexer_factory.build(chars.into_iter());
                let parser = Parser::new(self.interner, lexer);
                parser.collect()
            }
            SourceText::File(text) => {
                let lexer = self.lexer_factory.build(text);
                let parser = Parser::new(self.interner, lexer);
                parser.collect()
            },
        }
    }
}