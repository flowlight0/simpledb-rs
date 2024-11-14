use crate::{plan::Plan, record::field::Value, scan::Scan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    I32Constant(i32),
    StringConstant(String),
    Field(String),
}

impl Expression {
    pub fn evaluate(&self, scan: &mut Box<dyn Scan>) -> Result<Value, anyhow::Error> {
        match self {
            Expression::I32Constant(value) => Ok(Value::I32(*value)),
            Expression::StringConstant(value) => Ok(Value::String(value.clone())),
            Expression::Field(field_name) => {
                if scan.has_field(field_name) {
                    scan.get_value(field_name)
                } else {
                    Err(anyhow::anyhow!("Field {} not found", field_name))
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Equality(Expression, Expression),
}

impl Term {
    pub fn is_satisfied(&self, scan: &mut Box<dyn Scan>) -> Result<bool, anyhow::Error> {
        match self {
            Term::Equality(lhs, rhs) => {
                let lhs = lhs.evaluate(scan)?;
                let rhs = rhs.evaluate(scan)?;
                Ok(lhs == rhs)
            }
        }
    }

    pub fn get_reduction_factor(&self, plan: &Box<dyn Plan>) -> usize {
        todo!()
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

    pub fn is_satisfied(&self, scan: &mut Box<dyn Scan>) -> Result<bool, anyhow::Error> {
        for term in &self.terms {
            if !term.is_satisfied(scan)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub fn get_reduction_factor(&self, plan: &Box<dyn Plan>) -> usize {
        let mut factor = 1;
        for term in &self.terms {
            factor *= term.get_reduction_factor(plan);
        }
        factor
    }
}
