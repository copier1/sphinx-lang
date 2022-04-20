use crate::runtime::Variant;
use crate::runtime::strings::StringValue;
use crate::runtime::types::{Type, MetaObject};
use crate::runtime::errors::{ExecResult};


impl MetaObject for StringValue {
    fn type_tag(&self) -> Type { Type::String }
    
    fn len(&self) -> Option<ExecResult<usize>> {
        Some(Ok(self.char_len()))
    }
    
    fn op_add(&self, rhs: &Variant) -> Option<ExecResult<Variant>> {
        if let Some(rhs) = rhs.as_strval() {
            return Some(self.concat(&rhs).map(Variant::from))
        }
        None
    }
    
    fn op_radd(&self, lhs: &Variant) -> Option<ExecResult<Variant>> {
        if let Some(lhs) = lhs.as_strval() {
            return Some(lhs.concat(self).map(Variant::from))
        }
        None
    }
    
    fn cmp_eq(&self, other: &Variant) -> Option<ExecResult<bool>> {
        if let Some(other) = other.as_strval() {
            return Some(Ok(*self == other))
        }
        None
    }
    
    fn cmp_lt(&self, other: &Variant) -> Option<ExecResult<bool>> {
        if let Some(other) = other.as_strval() {
            return Some(Ok(*self < other))
        }
        None
    }
    
    fn cmp_le(&self, other: &Variant) -> Option<ExecResult<bool>> {
        if let Some(other) = other.as_strval() {
            return Some(Ok(*self <= other))
        }
        None
    }
}