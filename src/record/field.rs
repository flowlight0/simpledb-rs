use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub enum Spec {
    I32,
    VarChar(usize),
}

pub enum Type {
    I32,
    String,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord)]
pub enum Value {
    Null,
    I32(i32),
    String(String),
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Null, Value::Null) => Some(Ordering::Equal),
            (Value::I32(a), Value::I32(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => panic!(
                "Cannot compare different value variants: {:?} and {:?}. Such comparison should be avoided by the caller.",
                self, other
            ),
        }
    }
}
