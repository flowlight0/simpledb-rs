use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::field::Value,
    scan::{Scan, ScanControl},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    I32Constant(i32),
    StringConstant(String),
    Field(String),
}

impl Expression {
    pub fn evaluate(&self, scan: &mut Scan) -> Result<Value, TransactionError> {
        match self {
            Expression::I32Constant(value) => Ok(Value::I32(*value)),
            Expression::StringConstant(value) => Ok(Value::String(value.clone())),
            Expression::Field(field_name) => {
                if scan.has_field(field_name) {
                    scan.get_value(field_name)
                } else {
                    panic!("Field {} not found", field_name)
                }
            }
        }
    }

    pub fn try_get_field(&self) -> Option<&str> {
        match self {
            Expression::Field(field_name) => Some(field_name),
            _ => None,
        }
    }

    pub fn try_get_constant(&self) -> Option<Value> {
        match self {
            Expression::I32Constant(value) => Some(Value::I32(*value)),
            Expression::StringConstant(value) => Some(Value::String(value.clone())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Equality(Expression, Expression),
}

impl Term {
    pub fn is_satisfied(&self, scan: &mut Scan) -> Result<bool, TransactionError> {
        match self {
            Term::Equality(lhs, rhs) => {
                let lhs = lhs.evaluate(scan)?;
                let rhs = rhs.evaluate(scan)?;
                Ok(lhs == rhs)
            }
        }
    }

    pub fn get_reduction_factor(&self, plan: &Plan) -> usize {
        match self {
            Term::Equality(lhs, rhs) => {
                let lhs_field = lhs.try_get_field();
                let rhs_field = rhs.try_get_field();
                match (lhs_field, rhs_field) {
                    (Some(lhs_field), Some(rhs_field)) => {
                        let l_distinct = plan.num_distinct_values(lhs_field);
                        let r_distinct = plan.num_distinct_values(rhs_field);
                        return std::cmp::max(l_distinct, r_distinct);
                    }
                    (Some(lhs_field), None) => {
                        return plan.num_distinct_values(lhs_field);
                    }
                    (None, Some(rhs_field)) => {
                        return plan.num_distinct_values(rhs_field);
                    }
                    _ => {}
                }
                if lhs == rhs {
                    return 1;
                } else {
                    // Just return some relatively large number.
                    // Integer.MAX_VALUE was used in the book, but I don't use it to avoid overflow.
                    return 100;
                }
            }
        }
    }

    fn equates_with_constant(&self, field_name: &str) -> Option<Value> {
        match self {
            Term::Equality(lhs, rhs) => {
                if let Some(lhs_field) = lhs.try_get_field() {
                    if lhs_field == field_name {
                        if let Some(rhs_constant) = rhs.try_get_constant() {
                            return Some(rhs_constant);
                        }
                    }
                }
                if let Some(rhs_field) = rhs.try_get_field() {
                    if rhs_field == field_name {
                        if let Some(lhs_constant) = lhs.try_get_constant() {
                            return Some(lhs_constant);
                        }
                    }
                }
            }
        }
        return None;
    }

    fn equates_with_field(&self, field_name: &str) -> Option<String> {
        match self {
            Term::Equality(lhs, rhs) => {
                if let Some(lhs_field) = lhs.try_get_field() {
                    if lhs_field == field_name {
                        if let Some(rhs_field) = rhs.try_get_field() {
                            return Some(rhs_field.to_string());
                        }
                    }
                }
                if let Some(rhs_field) = rhs.try_get_field() {
                    if rhs_field == field_name {
                        if let Some(lhs_field) = lhs.try_get_field() {
                            return Some(lhs_field.to_string());
                        }
                    }
                }
            }
        }
        return None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    pub fn new(terms: Vec<Term>) -> Self {
        Predicate { terms }
    }

    pub fn is_satisfied(&self, scan: &mut Scan) -> Result<bool, TransactionError> {
        for term in &self.terms {
            if !term.is_satisfied(scan)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn get_reduction_factor(&self, plan: &Plan) -> usize {
        let mut factor = 1;
        for term in &self.terms {
            factor *= term.get_reduction_factor(plan);
        }
        factor
    }

    pub(crate) fn equates_with_constant(&self, field_name: &str) -> Option<Value> {
        for term in &self.terms {
            let t = term.equates_with_constant(field_name);
            if t.is_some() {
                return t;
            }
        }
        None
    }

    pub(crate) fn equates_with_field(&self, field_name: &str) -> Option<String> {
        for term in &self.terms {
            let t = term.equates_with_field(field_name);
            if t.is_some() {
                return t;
            }
        }
        None
    }
}
