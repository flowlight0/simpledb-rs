use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError, plan::PlanControl, record::schema::Schema, scan::Scan,
    tx::transaction::Transaction,
};

pub struct MultiBufferProductPlan {}

impl PlanControl for MultiBufferProductPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        todo!()
    }

    fn get_num_output_records(&self) -> usize {
        todo!()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        todo!()
    }

    fn schema(&self) -> &Schema {
        todo!()
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        todo!()
    }
}
