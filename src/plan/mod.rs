use std::sync::{Arc, Mutex};

use crate::{record::schema::Schema, scan::Scan, tx::transaction::Transaction};
pub mod select_plan;
pub mod table_plan;

pub trait Plan {
    fn get_num_accessed_blocks(&self) -> usize;
    fn get_num_output_records(&self) -> usize;
    fn num_distinct_values(&self, field_name: &str) -> usize;
    fn schema(&self) -> Schema;
    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Box<dyn Scan>, anyhow::Error>;
}
