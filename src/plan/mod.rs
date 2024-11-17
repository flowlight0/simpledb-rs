use std::sync::{Arc, Mutex};

use crate::{
    parser::statement::{QueryData, UpdateCommand},
    record::schema::Schema,
    scan::Scan,
    tx::{errors::TransactionError, transaction::Transaction},
};
pub mod basic_query_planner;
pub mod basic_update_planner;
pub mod planner;
pub mod product_plan;
pub mod project_plan;
pub mod select_plan;
pub mod table_plan;

pub trait Plan {
    fn get_num_accessed_blocks(&self) -> usize;
    fn get_num_output_records(&self) -> usize;
    fn num_distinct_values(&self, field_name: &str) -> usize;
    fn schema(&self) -> &Schema;
    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Box<dyn Scan>, TransactionError>;
}

pub trait QueryPlanner {
    fn create_plan(
        &self,
        query: &QueryData,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Box<dyn Plan>, TransactionError>;
}

pub trait UpdatePlanner {
    fn execute_update(
        &self,
        update_command: &UpdateCommand,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError>;
}
