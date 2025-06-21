use crate::{
    errors::TransactionError,
    record::{field::Value, schema::Schema},
    scan::{Scan, ScanControl},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    NullConstant,
    I32Constant(i32),
    StringConstant(String),
    Field(String),
    Add(Box<Expression>, Box<Expression>),
    Sub(Box<Expression>, Box<Expression>),
    Mul(Box<Expression>, Box<Expression>),
    Div(Box<Expression>, Box<Expression>),
}

impl Expression {
    /// Evaluates the expression against the given [`Scan`].
    ///
    /// The resulting [`Value`] is returned in a `Result` since accessing a
    /// field may fail if the underlying scan encounters an error.
    pub fn evaluate(&self, scan: &mut Scan) -> Result<Value, TransactionError> {
        match self {
            Expression::NullConstant => Ok(Value::Null),
            Expression::I32Constant(value) => Ok(Value::I32(*value)),
            Expression::StringConstant(value) => Ok(Value::String(value.clone())),
            Expression::Field(field_name) => {
                if scan.has_field(field_name) {
                    scan.get_value(field_name)
                } else {
                    panic!("Field {} not found", field_name)
                }
            }
            Expression::Add(lhs, rhs) => {
                let l = lhs.evaluate(scan)?;
                let r = rhs.evaluate(scan)?;
                match (l, r) {
                    (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a + b)),
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => panic!("Type mismatch in addition"),
                }
            }
            Expression::Sub(lhs, rhs) => {
                let l = lhs.evaluate(scan)?;
                let r = rhs.evaluate(scan)?;
                match (l, r) {
                    (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a - b)),
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => panic!("Type mismatch in subtraction"),
                }
            }
            Expression::Mul(lhs, rhs) => {
                let l = lhs.evaluate(scan)?;
                let r = rhs.evaluate(scan)?;
                match (l, r) {
                    (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a * b)),
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => panic!("Type mismatch in multiplication"),
                }
            }
            Expression::Div(lhs, rhs) => {
                let l = lhs.evaluate(scan)?;
                let r = rhs.evaluate(scan)?;
                match (l, r) {
                    (Value::I32(_), Value::I32(0)) => Ok(Value::Null),
                    (Value::I32(a), Value::I32(b)) => Ok(Value::I32(a / b)),
                    (Value::Null, _) | (_, Value::Null) => Ok(Value::Null),
                    _ => panic!("Type mismatch in division"),
                }
            }
        }
    }

    pub(crate) fn try_get_field(&self) -> Option<&str> {
        match self {
            Expression::Field(field_name) => Some(field_name),
            _ => None,
        }
    }

    pub(crate) fn try_get_constant(&self) -> Option<Value> {
        match self {
            Expression::NullConstant => Some(Value::Null),
            Expression::I32Constant(value) => Some(Value::I32(*value)),
            Expression::StringConstant(value) => Some(Value::String(value.clone())),
            Expression::Add(lhs, rhs) => match (lhs.try_get_constant(), rhs.try_get_constant()) {
                (Some(Value::I32(a)), Some(Value::I32(b))) => Some(Value::I32(a + b)),
                (Some(Value::Null), _) | (_, Some(Value::Null)) => Some(Value::Null),
                _ => None,
            },
            Expression::Sub(lhs, rhs) => match (lhs.try_get_constant(), rhs.try_get_constant()) {
                (Some(Value::I32(a)), Some(Value::I32(b))) => Some(Value::I32(a - b)),
                (Some(Value::Null), _) | (_, Some(Value::Null)) => Some(Value::Null),
                _ => None,
            },
            Expression::Mul(lhs, rhs) => match (lhs.try_get_constant(), rhs.try_get_constant()) {
                (Some(Value::I32(a)), Some(Value::I32(b))) => Some(Value::I32(a * b)),
                (Some(Value::Null), _) | (_, Some(Value::Null)) => Some(Value::Null),
                _ => None,
            },
            Expression::Div(lhs, rhs) => match (lhs.try_get_constant(), rhs.try_get_constant()) {
                (Some(Value::I32(_)), Some(Value::I32(0))) => Some(Value::Null),
                (Some(Value::I32(a)), Some(Value::I32(b))) => Some(Value::I32(a / b)),
                (Some(Value::Null), _) | (_, Some(Value::Null)) => Some(Value::Null),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns `true` if the expression can be applied to the given schema.
    ///
    /// Field references must exist in the schema while constant expressions are
    /// always applicable.
    pub(crate) fn is_applied_to(&self, schema: &Schema) -> bool {
        match self {
            Expression::Field(field_name) => schema.has_field(field_name),
            Expression::Add(lhs, rhs)
            | Expression::Sub(lhs, rhs)
            | Expression::Mul(lhs, rhs)
            | Expression::Div(lhs, rhs) => lhs.is_applied_to(schema) && rhs.is_applied_to(schema),
            _ => true,
        }
    }
}
