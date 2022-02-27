use std::fmt;
use std::error::Error;
use crate::source::ModuleSource;
use crate::lexer::{Span, TokenMeta};
use crate::debug::symbol::{DebugSymbol, TokenIndex};


pub type ErrorKind = ParserErrorKind;

// Specifies the actual error that occurred
#[derive(Debug)]
pub enum ParserErrorKind {
    LexerError,
    ExpectedStartOfExpr,   // expected the start of an expression
    ExpectedCloseParen,
    ExpectedCloseSquare,
    ExpectedCloseBrace,
    ExpectedIdentifier,
    InvalidAssignmentLHS,   // the LHS of an assignment was not a valid lvalue
}

impl ParserErrorKind {
    pub fn message(&self) -> &'static str {
        match self {
            Self::LexerError => "could not parse token",
            Self::ExpectedStartOfExpr => "expected start of expression",
            Self::ExpectedCloseParen => "expected closing ')'",
            Self::ExpectedCloseSquare => "expected closing ']'",
            Self::ExpectedCloseBrace => "expected closing '}'",
            Self::ExpectedIdentifier => "expected an identifier",
            Self::InvalidAssignmentLHS => "the left hand side of an assignment was invalid",
        }
    }
}

// Provide information about the type of syntactic construct from which the error originated
#[derive(Debug, Clone, Copy)]
pub enum ContextTag {
    Token,  // errors retrieving the actual tokens
    Expr,
    Statement,
    AssignmentExpr,
    BinaryOpExpr,
    UnaryOpExpr,
    PrimaryExpr,
    MemberAccess,
    IndexAccess,
    ObjectCtor,
    TupleCtor,
    Atom,
    Group,
}

// Since ErrorContext can share references with the Parser, we need to use 
// an error type that does not refer to the error context internally.
// The error context is always available at the base of the recursive descent call stack and can be added later.
#[derive(Debug)]
pub struct ErrorPrototype {
    kind: ErrorKind,
    cause: Option<Box<dyn Error>>,
}

impl ErrorPrototype {
    pub fn new(kind: ErrorKind) -> Self {
        ErrorPrototype { kind, cause: None }
    }
    
    pub fn caused_by(error: Box<dyn Error>, kind: ErrorKind) -> Self {
        ErrorPrototype { kind, cause: Some(error) }
    }
}

#[derive(Debug)]
pub struct ParserError<'m> {
    kind: ErrorKind,
    module: &'m ModuleSource,
    frame: ContextFrame,
    cause: Option<Box<dyn Error>>,
}

impl<'m> ParserError<'m> {
    pub fn from_prototype(proto: ErrorPrototype, context: ErrorContext<'m>) -> Self {
        ParserError {
            kind: proto.kind,
            module: context.module,
            frame: context.take(),
            cause: proto.cause,
        }
    }
    
    pub fn kind(&self) -> &ErrorKind { &self.kind }
}


impl Error for ParserError<'_> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|o| o.as_ref())
    }
}

impl fmt::Display for ParserError<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt.write_str(self.kind.message())?;
        if let Some(err) = self.source() {
            write!(fmt, ": {}", err)?;
        }
        Ok(())
    }
}


// Structures used by the parser for error handling and synchronization

#[derive(Debug, Clone)]
pub struct ErrorContext<'m> {
    module: &'m ModuleSource,
    stack: Vec<ContextFrame>,
}

impl<'m> ErrorContext<'m> {
    pub fn new(module: &'m ModuleSource, base: ContextTag) -> Self {
        ErrorContext {
            module, stack: vec![ ContextFrame::new(base) ],
        }
    }
    
    //pub fn module(&self) -> &'m str { self.module }
    
    pub fn frame(&self) -> &ContextFrame { self.stack.last().unwrap() }
    pub fn frame_mut(&mut self) -> &mut ContextFrame { self.stack.last_mut().unwrap() }
    
    pub fn push(&mut self, tag: ContextTag) { self.stack.push(ContextFrame::new(tag)) }
    
    pub fn push_continuation(&mut self, tag: ContextTag) {
        let start = self.frame().start().map(|o| o.to_owned());
        self.push(tag);
        self.frame_mut().set_span(start, None);
    }
    
    pub fn pop(&mut self) -> ContextFrame { 
        assert!(self.stack.len() > 1);
        self.stack.pop().unwrap()
    }
    
    pub fn pop_extend(&mut self) {
        let inner_frame = self.pop();
        self.frame_mut().extend(inner_frame);
    }
    
    pub fn take(mut self) -> ContextFrame {
        assert!(!self.stack.is_empty());
        self.stack.pop().unwrap()
    }
    
    // for convenience
    pub fn context(&self) -> ContextTag { self.frame().context() }
    pub fn set_start(&mut self, token: &TokenMeta) { self.frame_mut().set_start(token) }
    pub fn set_end(&mut self, token: &TokenMeta) { self.frame_mut().set_end(token) }
}

#[derive(Debug, Clone)]
pub struct ContextFrame {
    tag: ContextTag,
    start: Option<Span>,
    end: Option<Span>,
}

fn span_lt(first: &Span, second: &Span) -> bool { first.index < second.index }
// fn span_min<'m>(first: &'m Span, second: &'m Span) -> &'m Span {
//     if span_lt(first, second) { first } else { second }
// }
// fn span_max<'m>(first: &'m Span, second: &'m Span) -> &'m Span {
//     if !span_lt(first, second) { first } else { second }
// }

impl ContextFrame {
    pub fn new(tag: ContextTag) -> Self { ContextFrame { tag, start: None, end: None } }
    
    pub fn context(&self) -> ContextTag { self.tag }
    pub fn start(&self) -> Option<&Span> { self.start.as_ref() }
    pub fn end(&self) -> Option<&Span> { self.end.as_ref() }
    
    pub fn set_start(&mut self, token: &TokenMeta) { 
        self.start.replace(token.span.clone()); 
    }
    
    pub fn set_end(&mut self, token: &TokenMeta) { 
        self.end.replace(token.span.clone()); 
    }
    
    pub fn set_span(&mut self, start: Option<Span>, end: Option<Span>) {
        self.start = start;
        self.end = end;
    }
    
    pub fn extend(&mut self, other: ContextFrame) {
        if self.start.as_ref().and(other.start.as_ref()).is_some() {
            if span_lt(other.start.as_ref().unwrap(), self.start.as_ref().unwrap()) {
                self.start = other.start;
            }
        } else if other.start.is_some() {
            self.start = other.start;
        }
        
        if self.end.as_ref().and(other.end.as_ref()).is_some() {
            if span_lt(self.end.as_ref().unwrap(), other.end.as_ref().unwrap()) {
                self.end = other.end;
            }
        } else if other.end.is_some() {
            self.end = other.end;
        }
    }
}

impl From<&ContextFrame> for DebugSymbol {
    fn from(frame: &ContextFrame) -> Self {
        match (frame.start.clone(), frame.end.clone()) {
            (Some(start), Some(end)) => {
                let start_index = start.index;
                let end_index = end.index + TokenIndex::from(end.length);
                
                (start_index, end_index).into()
            },
            (Some(span), None) | (None, Some(span)) => {
                let start_index = span.index;
                let end_index = span.index + TokenIndex::from(span.length);
                
                (start_index, end_index).into()
            },
            (None, None) => {
                panic!("ContextFrame has no source index information");
            }
        }
    }
}
