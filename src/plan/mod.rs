use std::sync::{Arc, Mutex};

use enum_dispatch::enum_dispatch;
use product_plan::ProductPlan;
use project_plan::ProjectPlan;
use select_plan::SelectPlan;
use table_plan::TablePlan;

use crate::{
    errors::TransactionError,
    index::plan::{index_join_plan::IndexJoinPlan, index_select_plan::IndexSelectPlan},
    materialization::{
        group_by_plan::GroupByPlan, materialize_plan::MaterializePlan, sort_plan::SortPlan,
    },
    parser::statement::{QueryData, UpdateCommand},
    record::schema::Schema,
    scan::Scan,
    tx::transaction::Transaction,
};
pub mod basic_query_planner;
pub mod basic_update_planner;
pub mod planner;
pub mod product_plan;
pub mod project_plan;
pub mod select_plan;
pub mod table_plan;

#[enum_dispatch]
pub enum Plan {
    ProductPlan(ProductPlan),
    ProjectPlan(ProjectPlan),
    SelectPlan(SelectPlan),
    TablePlan(TablePlan),
    IndexJoinPlan(IndexJoinPlan),
    IndexSelectPlan(IndexSelectPlan),
    MaterializePlan(MaterializePlan),
    SortPlan(SortPlan),
    GroupByPlan(GroupByPlan),
}

#[enum_dispatch(Plan)]
pub trait PlanControl {
    // Return the number of blocks accessed by the plan
    fn get_num_accessed_blocks(&self) -> usize;

    // Return the number of output records of the scan
    fn get_num_output_records(&self) -> usize;

    // Return the estimated number of distinct values for the given field
    fn num_distinct_values(&self, field_name: &str) -> usize;

    // Return the schema of the output scan
    fn schema(&self) -> &Schema;

    // Open the plan and return the scan
    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError>;
}

pub trait QueryPlanner: Send + Sync {
    fn create_plan(
        &self,
        query: &QueryData,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Plan, TransactionError>;
}

pub trait UpdatePlanner: Send + Sync {
    fn execute_update(
        &self,
        update_command: &UpdateCommand,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<usize, TransactionError>;
}
