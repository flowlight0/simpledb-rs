use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    metadata::index_manager::IndexInfo,
    plan::{table_plan::TablePlan, Plan},
    record::{field::Value, schema::Schema},
    scan::Scan,
    tx::transaction::Transaction,
};

pub struct IndexSelectPlan {
    table_plan: TablePlan,
    index_info: IndexInfo,
    value: Value,
}

impl IndexSelectPlan {
    pub fn new(table_plan: TablePlan, index_info: IndexInfo, value: &Value) -> Self {
        IndexSelectPlan {
            table_plan,
            index_info,
            value: value.clone(),
        }
    }
}

impl Plan for IndexSelectPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.index_info.get_num_accessed_blocks() + self.get_num_output_records()
    }

    fn get_num_output_records(&self) -> usize {
        self.index_info.get_num_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.index_info.get_distinct_values(field_name)
    }

    fn schema(&self) -> &Schema {
        self.table_plan.schema()
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Box<dyn Scan>, TransactionError> {
        todo!()
    }
}
