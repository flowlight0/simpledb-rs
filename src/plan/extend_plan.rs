use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    parser::expression::Expression,
    record::{field::Spec, schema::Schema},
    scan::{extend_scan::ExtendScan, Scan},
    tx::transaction::Transaction,
};

use super::{Plan, PlanControl};

#[derive(Clone)]
pub struct ExtendPlan {
    plan: Box<Plan>,
    expression: Expression,
    field_name: String,
    schema: Schema,
}

impl ExtendPlan {
    pub fn new(plan: Plan, expression: Expression, field_name: &str) -> Self {
        let mut schema = plan.schema().clone();
        let spec = Self::infer_spec(&expression, plan.schema());
        schema.add_field(field_name, &spec);
        Self {
            plan: Box::new(plan),
            expression,
            field_name: field_name.to_string(),
            schema,
        }
    }

    fn infer_spec(expr: &Expression, schema: &Schema) -> Spec {
        match expr {
            Expression::Field(f) => schema.get_field_spec(f),
            Expression::StringConstant(s) => Spec::VarChar(s.len()),
            Expression::Add(_, _)
            | Expression::Sub(_, _)
            | Expression::Mul(_, _)
            | Expression::Div(_, _)
            | Expression::I32Constant(_)
            | Expression::NullConstant => Spec::I32,
        }
    }
}

impl PlanControl for ExtendPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.plan.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.plan.get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if field_name == self.field_name {
            self.plan.get_num_output_records()
        } else {
            self.plan.num_distinct_values(field_name)
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let scan = self.plan.open(tx)?;
        Ok(Scan::from(ExtendScan::new(
            scan,
            self.expression.clone(),
            &self.field_name,
        )))
    }
}
