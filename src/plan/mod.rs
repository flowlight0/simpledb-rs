use crate::{record::schema::Schema, scan::Scan, tx::transaction::Transaction};
mod table_plan;

pub trait Plan {
    fn get_num_accessed_blocks(&self) -> usize;
    fn get_num_output_records(&self) -> usize;
    fn num_distinct_values(&self, field_name: &str) -> usize;
    fn schema(&self) -> Schema;
    fn open<'a>(&'a mut self, tx: &'a mut Transaction)
        -> Result<Box<dyn Scan + 'a>, anyhow::Error>;
}
