#[derive(Debug, PartialEq, Eq)]
pub enum Spec {
    I32,
    VarChar(usize),
}

pub enum Type {
    I32,
    String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    I32(i32),
    String(String),
}
