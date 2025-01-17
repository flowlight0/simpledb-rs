use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    record::schema::Schema,
    scan::{project_scan::ProjectScan, Scan},
    tx::transaction::Transaction,
};

use super::{Plan, PlanControl};

#[derive(Clone)]
pub struct ProjectPlan {
    pub plan: Box<Plan>,
    pub schema: Schema,
}

impl ProjectPlan {
    pub fn new(plan: Plan, fields: Vec<String>) -> Self {
        let mut schema = Schema::new();
        for field in &fields {
            let field_spec = &plan.schema().get_field_spec(field);
            schema.add_field(field, field_spec);
        }
        ProjectPlan {
            plan: Box::new(plan),
            schema,
        }
    }
}

impl PlanControl for ProjectPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.plan.get_num_accessed_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.plan.get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.plan.num_distinct_values(field_name)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let scan = self.plan.open(tx)?;
        Ok(Scan::from(ProjectScan::new(scan, self.schema.get_fields())))
    }
}
