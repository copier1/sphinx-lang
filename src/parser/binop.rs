use std::fmt;

// Binary Operator

#[derive(Clone, Copy, Debug)]
pub enum BinaryOp {
    // precedence level 2
    Mul, Div, Mod,
    
    // precedence level 3
    Add, Sub,
    
    // precedence level 4
    LShift, RShift,
    
    // precedence level 5
    BitAnd,
    
    // precedence level 6
    BitXor,
    
    // precedence level 7
    BitOr,
    
    // precedence level 8
    LT, GT, LE, GE, EQ, NE,
    
    // precedence level 9
    And,
    
    // precedence level 10
    Or,
}

impl BinaryOp {
    pub fn precedence_level(&self) -> i8 {
        match self {
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 2,
            
            BinaryOp::Add | BinaryOp::Sub => 3,
            
            BinaryOp::LShift | BinaryOp::RShift => 4,
            
            BinaryOp::BitAnd => 5,
            BinaryOp::BitXor => 6,
            BinaryOp::BitOr => 7,
            
            BinaryOp::LT | BinaryOp::GT | BinaryOp::LE 
            | BinaryOp::GE | BinaryOp::EQ | BinaryOp::NE => 8,
            
            BinaryOp::And => 9,
            BinaryOp::Or => 10,
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let symbol = match self {
            BinaryOp::Mul    => "*", 
            BinaryOp::Div    => "/", 
            BinaryOp::Mod    => "%",
            BinaryOp::Add    => "+",
            BinaryOp::Sub    => "-",
            BinaryOp::LShift => "<<", 
            BinaryOp::RShift => ">>",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitXor => "^",
            BinaryOp::BitOr  => "|",
            BinaryOp::LT     => "<",
            BinaryOp::GT     => ">",
            BinaryOp::LE     => "<=",
            BinaryOp::GE     => ">=",
            BinaryOp::EQ     => "==",
            BinaryOp::NE     => "!=",
            BinaryOp::And    => "and",
            BinaryOp::Or     => "or",
        };
        fmt.write_str(symbol)
    }
}


