use std::fmt;
use std::rc::Rc;
use std::hash::{Hash, Hasher};
use std::cmp::{PartialEq, Eq};
use crate::language::{IntType, FloatType};
use crate::runtime::types::Metatable;
use crate::runtime::types::primitive::*;
use crate::runtime::strings::{StringSymbol, STRING_TABLE};
use crate::runtime::errors::{ExecResult, RuntimeError, ErrorKind};


// Temporary until a proper type system is implemented?
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeTag {
    Nil,
    Tuple,
    Bool,
    Integer,
    Float,
    String,
    Object,
}

// Fundamental data value type
#[derive(Clone)] // add Copy?
pub enum Variant {
    Nil,
    EmptyTuple, // the empty tuple value
    BoolTrue,
    BoolFalse,
    Integer(IntType),
    Float(FloatType),
    String(StringSymbol),
    // TODO just GC tuples
    Tuple(Rc<[Variant]>),  //  will use COW semantics, so if we need to send to another thread we can just clone the underlying data
    //Object(GCHandle),
}

impl Variant {
    //TODO type system
    pub fn type_tag(&self) -> TypeTag {
        match self {
            Self::Nil => TypeTag::Nil,
            Self::BoolTrue | Self::BoolFalse => TypeTag::Bool,
            Self::Integer(..) => TypeTag::Integer,
            Self::Float(..) => TypeTag::Float,
            Self::String(..) => TypeTag::String,
            Self::EmptyTuple | Self::Tuple(..) => TypeTag::Tuple,
        }
    }
    
    pub fn metatable(&self) -> &Metatable {
        match self {
            Self::String(..) => &METATABLE_STRING,
            _ => &METATABLE_DEFAULT,
        }
    }
    
    // Only "nil" and "false" have a truth value of false.
    pub fn truth_value(&self) -> bool {
        !matches!(self, Self::Nil | Self::BoolFalse)
    }
    
    // Note, bit_value() and float_value() are defined based on what is needed for the language *implementation*
    // They do not reflect the semantics of the Sphinx language
    
    pub fn bit_value(&self) -> Option<IntType> {
        let value = match self {
            Self::Integer(value) => *value,
            Self::BoolFalse => 0, // all 0s
            Self::BoolTrue => !0, // all 1s
            _ => return None,
        };
        Some(value)
    }
    
    pub fn float_value(&self) -> Option<FloatType> {
        let value = match self {
            // it's okay if this is a lossy conversion
            Self::Integer(value) => (*value) as FloatType,
            Self::Float(value) => *value,
            _ => return None,
        };
        Some(value)
    }
    
    pub fn make_tuple(items: Box<[Variant]>) -> Self {
        Self::Tuple(Rc::from(items))
    }
    
    pub fn can_hash(&self) -> bool {
        match self {
            Self::Float(..) => false,
            Self::Tuple(items) => items.iter().all(|item| item.can_hash()),
            // TODO Objects - check metatabale for __hash
            _ => true
        }
        
    }
}

impl From<bool> for Variant {
    fn from(value: bool) -> Self {
        match value {
            true => Self::BoolTrue,
            false => Self::BoolFalse,
        }
    }
}

impl From<IntType> for Variant {
    fn from(value: IntType) -> Self { Variant::Integer(value) }
}

impl From<FloatType> for Variant {
    fn from(value: FloatType) -> Self { Variant::Float(value) }
}

impl From<StringSymbol> for Variant {
    fn from(value: StringSymbol) -> Self { Variant::String(value) }
}


const AUTO_INTERN_MAX: usize = 40;

impl From<&str> for Variant {
    fn from(value: &str) -> Self {
        if value.len() <= AUTO_INTERN_MAX {
            return STRING_TABLE.with(|string_table| string_table.get_or_intern(value)).into();
        }
        
        if let Some(symbol) = STRING_TABLE.with(|string_table| string_table.get(value)) {
            return symbol.into();
        }
        
        // long strings
        unimplemented!()
    }
}


// Not all Variants are hashable, so there is a separate type to handle that

#[derive(Clone)]
pub struct VariantKey<'a>(&'a Variant);

impl<'a> TryFrom<&'a Variant> for VariantKey<'a> {
    type Error = RuntimeError;
    fn try_from(value: &'a Variant) -> ExecResult<Self> {
        if !value.can_hash() {
            return Err(ErrorKind::UnhashableValue(value.clone()).into());
        }
        Ok(Self(&value))
    }
}

impl Hash for VariantKey<'_> {
    fn hash<H>(&self, state: &mut H) where H: Hasher {
        debug_assert!(self.0.can_hash());
        
        let discriminant = std::mem::discriminant(self.0);
        discriminant.hash(state);
        
        match self.0 {
            Variant::Integer(value) => value.hash(state),
            Variant::String(strkey) => strkey.hash(state),
            // TODO objects - get the result of __hash and hash it
            _ => { }
        }
    }
}

impl<'s> PartialEq for VariantKey<'_> {
    fn eq(&self, other: &VariantKey) -> bool {
        match (self.0, other.0) {
            (Variant::Nil, Variant::Nil) => true,
            (Variant::EmptyTuple, Variant::EmptyTuple) => true,
            (Variant::BoolTrue, Variant::BoolTrue) => true,
            (Variant::BoolFalse, Variant::BoolFalse) => true,
            
            (Variant::Integer(a), Variant::Integer(b)) => a == b,
            (Variant::String(a), Variant::String(b)) => a == b,
            
            (Variant::Tuple(a), Variant::Tuple(b)) if a.len() != b.len() => false,
            (Variant::Tuple(a), Variant::Tuple(b)) => {
                a.iter().zip(b.iter())
                .all(|(a, b)| VariantKey(a) == VariantKey(b))
            },
            
            // TODO objects, use __eq
            
            _ => false,
        }
    }
}
impl Eq for VariantKey<'_> { }

impl fmt::Debug for Variant {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nil => fmt.write_str("nil"),
            Self::EmptyTuple => fmt.write_str("()"),
            Self::BoolTrue => fmt.write_str("true"),
            Self::BoolFalse => fmt.write_str("false"),
            
            Self::Integer(value) => write!(fmt, "{}", *value),
            Self::Float(value) => {
                if value.trunc() != *value {
                    write!(fmt, "{}", *value)
                } else {
                    write!(fmt, "{}.0", value)
                }
            },
            
            Self::String(value) => write!(fmt, "\"{}\"", value),
            
            Self::Tuple(items) => {
                let (last, rest) = items.split_last().unwrap(); // will never be empty
                
                write!(fmt, "(")?;
                for item in rest.iter() {
                    write!(fmt, "{:?}, ", item)?;
                }
                write!(fmt, "{:?})", last)
            }
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => write!(fmt, "{}", value),
            
            Self::Tuple(items) => {
                let (last, rest) = items.split_last().unwrap(); // will never be empty
                
                write!(fmt, "(")?;
                for item in rest.iter() {
                    write!(fmt, "{}, ", item)?;
                }
                write!(fmt, "{})", last)
            }
            
            _ => write!(fmt, "{}", self)
        }
    }
}



