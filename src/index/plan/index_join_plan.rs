use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    index::scan::index_join_scan::IndexJoinScan,
    metadata::index_manager::IndexInfo,
    plan::{Plan, PlanControl},
    record::schema::Schema,
    scan::Scan,
    tx::transaction::Transaction,
};

#[derive(Clone)]
pub struct IndexJoinPlan {
    p1: Box<Plan>,
    p2: Box<Plan>,
    index_info: IndexInfo,
    join_field: String,
    schema: Schema,
}

impl IndexJoinPlan {
    pub fn new(p1: Plan, p2: Plan, index_info: IndexInfo, join_field: String) -> Self {
        let mut schema = Schema::new();
        schema.add_all(&p1.schema());
        schema.add_all(&p2.schema());
        Self {
            p1: Box::new(p1),
            p2: Box::new(p2),
            index_info,
            join_field,
            schema,
        }
    }
}

impl PlanControl for IndexJoinPlan {
    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let lhs = self.p1.open(tx.clone())?;
        let rhs = self.p2.open(tx.clone())?;
        match rhs {
            Scan::TableScan(rhs_ts) => {
                let rhs_index = self.index_info.open()?;
                let join_field = self.join_field.clone();
                Ok(Scan::from(IndexJoinScan::new(
                    lhs, rhs_index, join_field, rhs_ts,
                )?))
            }
            _ => panic!("Expected TableScan"),
        }
    }

    fn get_num_accessed_blocks(&self) -> usize {
        self.p1.get_num_accessed_blocks()
            + self.p1.get_num_output_records() * self.index_info.get_num_accessed_blocks()
            + self.get_num_output_records()
    }

    fn get_num_output_records(&self) -> usize {
        self.p1.get_num_output_records() * self.index_info.get_num_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        if self.p1.schema().has_field(field_name) {
            self.p1.num_distinct_values(field_name)
        } else {
            self.p2.num_distinct_values(field_name)
        }
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }
}
