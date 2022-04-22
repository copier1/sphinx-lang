use core::fmt;
use std::error::Error;

use crate::utils;
use crate::runtime::Variant;
use crate::runtime::function::Signature;
use crate::runtime::gc::GcTrace;
use crate::runtime::types::{Type, MethodTag};
use crate::runtime::strings::StringSymbol;
use crate::debug::traceback::{TraceSite, Traceback};

// TODO box error
pub type ExecResult<T> = Result<T, Box<RuntimeError>>;

#[derive(Debug)]
pub enum ErrorKind {
    // TODO replace variant with type name
    InvalidUnaryOperand(Type),  // unsupported operand for type
    InvalidBinaryOperand(Type, Type),
    OverflowError,
    DivideByZero,
    NegativeShiftCount,
    NameNotDefined(String),
    CantAssignImmutable,  // can't assign to immutable global variable
    UnhashableValue(Variant),
    MissingArguments { signature: Box<Signature>, nargs: usize },
    TooManyArguments { signature: Box<Signature>, nargs: usize },
    MethodNotSupported(Type, MethodTag),
    AssertFailed,
    InvalidValue(Variant, String),
    Other(String),
}

impl From<ErrorKind> for RuntimeError {
    fn from(kind: ErrorKind) -> Self {
        RuntimeError { kind, traceback: Vec::new(), cause: None }
    }
}

impl From<ErrorKind> for Box<RuntimeError> {
    fn from(kind: ErrorKind) -> Self {
        Box::new(kind.into())
    }
}

unsafe impl GcTrace for ErrorKind {
    fn trace(&self) {
        match self {
            Self::UnhashableValue(value) => value.trace(),
            _ => { },
        }
    }
    
    fn size_hint(&self) -> usize {
        match self {
            Self::MissingArguments { .. } => core::mem::size_of::<Signature>(),
            Self::TooManyArguments { .. } => core::mem::size_of::<Signature>(),
            _ => 0,
        }
    }
}



#[derive(Debug)]
pub struct RuntimeError {
    kind: ErrorKind,
    traceback: Vec<TraceSite>,
    cause: Option<Box<RuntimeError>>,
}

unsafe impl GcTrace for RuntimeError {
    fn trace(&self) {
        self.kind.trace();
        for site in self.traceback.iter() {
            site.trace();
        }
        if let Some(error) = self.cause.as_ref() {
            error.trace();
        }
    }
}

impl RuntimeError {
    pub fn caused_by(mut self: Box<Self>, cause: Box<RuntimeError>) -> Box<Self> {
        self.cause.replace(cause); self
    }
    
    pub fn extend_trace(mut self: Box<Self>, trace: impl Iterator<Item=TraceSite>) -> Box<Self> {
        self.traceback.extend(trace); self
    }
    
    pub fn push_frame(mut self: Box<Self>, site: TraceSite) -> Box<Self> {
        self.traceback.push(site); self
    }
    
    pub fn kind(&self) -> &ErrorKind { &self.kind }
    
    pub fn traceback(&self) -> Traceback<'_> {
        Traceback::build(self.traceback.iter())
    }
}

impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(
            |error| &*error as &RuntimeError as &dyn Error
        )
    }
}

#[allow(clippy::useless_format)]
impl fmt::Display for RuntimeError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let message = match self.kind() {
            // TODO
            ErrorKind::InvalidUnaryOperand(operand) => format!("unsupported operand: '{}'", operand),
            ErrorKind::InvalidBinaryOperand(lhs, rhs) => format!("unsupported operands: '{}' and '{}'", lhs, rhs),
            ErrorKind::DivideByZero => format!("divide by zero"),
            ErrorKind::OverflowError => format!("integer overflow"),
            ErrorKind::NegativeShiftCount => format!("negative bitshift count"),
            ErrorKind::NameNotDefined(name) => format!("undefined variable \"{}\"", name),
            ErrorKind::CantAssignImmutable => format!("can't assign to an immutable variable"),
            ErrorKind::UnhashableValue(value) => format!("{} is not hashable", value.echo()),
            ErrorKind::AssertFailed => format!("assertion failed"),
            ErrorKind::Other(message) => message.to_string(),
            
            ErrorKind::InvalidValue(value, message) => {
                if message.is_empty() {
                    format!("invalid value {}", value.echo())
                } else {
                    format!("invalid value {}: {}", value.echo(), message)
                }
            }
            
            ErrorKind::MethodNotSupported(receiver, method) => {
                match method {
                    MethodTag::AsBits => format!("can't interpret '{}' as bitfield", receiver),
                    MethodTag::AsInt => format!("can't interpret '{}' as int", receiver),
                    MethodTag::AsFloat => format!("can't interpret '{}' as float", receiver),
                    MethodTag::Invoke => format!("type '{}' is not callable", receiver),
                    MethodTag::Next => format!("type '{}' is not an iterator", receiver),
                    MethodTag::Iter => format!("type '{}' is not iterable", receiver),
                    _ => format!("type '{}' does not support '__{}'", receiver, method),
                }
            }
            
            ErrorKind::MissingArguments { signature, nargs } => {
                let missing = signature.required().iter()
                    .skip(*nargs)
                    .map(|param| *param.name())
                    .collect::<Vec<StringSymbol>>();
                
                let count = signature.min_arity() - nargs;
                
                format!(
                    "{} missing {} required {}: {}",
                    signature.display_short(), 
                    count, 
                    if count == 1 { "argument" }
                    else { "arguments" },
                    utils::fmt_join(", ", &missing),
                )
            },
            
            ErrorKind::TooManyArguments { signature, nargs } => {
                format!(
                    "{} takes {} arguments but {} were given", 
                    signature.display_short(), 
                    signature.max_arity().unwrap(), 
                    nargs,
                )
            },

        };
        
        utils::format_error(fmt, "Runtime error", Some(&message), self.source())
    }
}

/*
Probably declare these in the debug module...

pub struct Frame {
    symbol: DebugSymbol,
    context: ...,
}

pub struct Traceback {
    frames: Vec<Frame>,
}
*/