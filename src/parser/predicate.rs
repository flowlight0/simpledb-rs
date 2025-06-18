use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::{field::Value, schema::Schema},
    scan::{Scan, ScanControl},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expression {
    NullConstant,
    I32Constant(i32),
    StringConstant(String),
    Field(String),
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
            Expression::NullConstant => Some(Value::Null),
            Expression::I32Constant(value) => Some(Value::I32(*value)),
            Expression::StringConstant(value) => Some(Value::String(value.clone())),
            _ => None,
        }
    }

    /// Returns `true` if the expression can be applied to the given schema.
    ///
    /// Field references must exist in the schema while constant expressions are
    /// always applicable.
    fn is_applied_to(&self, schema: &Schema) -> bool {
        match self {
            Expression::Field(field_name) => schema.has_field(field_name),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    Equality(Expression, Expression),
    IsNull(Expression),
}

impl Term {
    /// Evaluates the term for the current row of the given [`Scan`].
    ///
    /// Returns `Ok(true)` if the term is satisfied, `Ok(false)` otherwise. Any
    /// I/O errors produced while reading values from the scan are forwarded.
    pub fn is_satisfied(&self, scan: &mut Scan) -> Result<bool, TransactionError> {
        match self {
            Term::Equality(lhs, rhs) => {
                let lhs = lhs.evaluate(scan)?;
                let rhs = rhs.evaluate(scan)?;
                Ok(lhs == rhs)
            }
            Term::IsNull(expr) => {
                let value = expr.evaluate(scan)?;
                Ok(value == Value::Null)
            }
        }
    }

    /// Estimates the reduction factor of the term for query planning.
    ///
    /// The reduction factor is based on the distinct value counts provided by
    /// the [`Plan`] and follows the same heuristics as the Java version from
    /// the "SimpleDB" textbook.
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
            Term::IsNull(expr) => {
                if let Some(field) = expr.try_get_field() {
                    return plan.num_distinct_values(field);
                }
                if let Some(value) = expr.try_get_constant() {
                    return if value == Value::Null { 1 } else { 100 };
                }
                100
            }
        }
    }

    /// If this term equates the given field with a constant, return that constant.
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
            Term::IsNull(expr) => {
                if let Some(field) = expr.try_get_field() {
                    if field == field_name {
                        return Some(Value::Null);
                    }
                }
            }
        }
        return None;
    }

    /// If this term equates the given field with another field, return the other
    /// field name.
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
            Term::IsNull(_) => {}
        }
        return None;
    }

    /// Returns `true` if all fields referenced by the term are present in the
    /// provided schema.
    fn is_applied_to(&self, schema: &Schema) -> bool {
        match self {
            Term::Equality(lhs, rhs) => lhs.is_applied_to(schema) && rhs.is_applied_to(schema),
            Term::IsNull(expr) => expr.is_applied_to(schema),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    /// Creates a new predicate composed of the given terms.
    pub fn new(terms: Vec<Term>) -> Self {
        Predicate { terms }
    }

    /// Checks whether all terms are satisfied for the current record of the scan.
    pub fn is_satisfied(&self, scan: &mut Scan) -> Result<bool, TransactionError> {
        for term in &self.terms {
            if !term.is_satisfied(scan)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Computes the combined reduction factor of all terms.
    ///
    /// This is the product of the individual reduction factors and is used by
    /// the planner to estimate result sizes.
    pub fn get_reduction_factor(&self, plan: &Plan) -> usize {
        let mut factor = 1;
        for term in &self.terms {
            factor *= term.get_reduction_factor(plan);
        }
        factor
    }

    /// Returns a predicate containing only the terms that can be applied to the
    /// provided schema.
    ///
    /// If no such terms exist, `None` is returned.
    pub(crate) fn select_sub_predicates(&self, schema: &Schema) -> Option<Predicate> {
        let mut terms = Vec::new();
        for term in &self.terms {
            if term.is_applied_to(schema) {
                terms.push(term.clone());
            }
        }

        if terms.is_empty() {
            return None;
        } else {
            Some(Predicate::new(terms))
        }
    }

    /// Extracts the join predicate that references fields from both schemas.
    ///
    /// Terms that reference only one of the schemas are ignored. If none remain
    /// after filtering, `None` is returned.
    pub(crate) fn join_sub_predicates(
        &self,
        schema1: &Schema,
        schema2: &Schema,
    ) -> Option<Predicate> {
        let mut join_schema = Schema::new();
        join_schema.add_all(schema1);
        join_schema.add_all(schema2);

        let mut terms = vec![];
        for term in &self.terms {
            if term.is_applied_to(schema1) || term.is_applied_to(schema2) {
                continue;
            }
            if term.is_applied_to(&join_schema) {
                terms.push(term.clone());
            }
        }
        if terms.is_empty() {
            return None;
        } else {
            Some(Predicate::new(terms))
        }
    }

    /// Searches for a term equating `field_name` with a constant value.
    pub(crate) fn equates_with_constant(&self, field_name: &str) -> Option<Value> {
        for term in &self.terms {
            let t = term.equates_with_constant(field_name);
            if t.is_some() {
                return t;
            }
        }
        None
    }

    /// Searches for a term equating `field_name` with another field.
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
