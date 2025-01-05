use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::schema::Schema,
    scan::{Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::{record_comparator::RecordComparator, sort_scan::SortScan, temp_table::TempTable};

pub struct SortPlan {
    base_plan: Box<Plan>,
    tx: Arc<Mutex<Transaction>>,
    schema: Schema,
    comparator: Arc<RecordComparator>,
}

impl SortPlan {
    pub fn new(
        base_plan: Plan,
        tx: Arc<Mutex<Transaction>>,
        schema: Schema,
        comparator: Arc<RecordComparator>,
    ) -> Self {
        Self {
            base_plan: Box::new(base_plan),
            tx,
            schema,
            comparator,
        }
    }

    fn split_into_runs(
        &self,
        mut base_scan: Scan,
        comparator: &RecordComparator,
    ) -> Result<Vec<TempTable>, TransactionError> {
        todo!()
    }

    fn merge_run_iteration(
        &self,
        runs: Vec<TempTable>,
    ) -> Result<Vec<TempTable>, TransactionError> {
        todo!()
    }
}

impl PlanControl for SortPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        todo!()
    }

    fn get_num_output_records(&self) -> usize {
        todo!()
    }

    fn num_distinct_values(&self, _field_name: &str) -> usize {
        todo!()
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, _tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let base_scan = self.base_plan.open(self.tx.clone())?;
        let mut runs: Vec<TempTable> = self.split_into_runs(base_scan, &self.comparator)?;

        while runs.len() > 1 {
            runs = self.merge_run_iteration(runs)?;
        }
        SortScan::new(runs, self.comparator.clone()).map(|scan| Scan::from(scan))
    }
}
