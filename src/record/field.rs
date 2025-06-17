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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    I32(i32),
    String(String),
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Value::Null, Value::Null) => Some(Ordering::Equal),
            (Value::Null, _) => Some(Ordering::Greater),
            (_, Value::Null) => Some(Ordering::Less),
            (Value::I32(a), Value::I32(b)) => a.partial_cmp(b),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            _ => panic!(
                "Cannot compare different value variants: {:?} and {:?}. Such comparison should be avoided by the caller.",
                self, other
            ),
        }
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Greater,
            (_, Value::Null) => Ordering::Less,
            (Value::I32(a), Value::I32(b)) => a.cmp(b),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            _ => panic!(
                "Cannot compare different value variants: {:?} and {:?}. Such comparison should be avoided by the caller.",
                self, other
            ),
        }
    }
}
