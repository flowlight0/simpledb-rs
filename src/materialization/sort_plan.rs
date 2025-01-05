use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::schema::{self, Schema},
    scan::{Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::{
    materialize_plan::MaterializePlan, record_comparator::RecordComparator, sort_scan::SortScan,
    temp_table::TempTable,
};

pub struct SortPlan {
    base_plan: RefCell<Option<Box<Plan>>>,
    tx: Arc<Mutex<Transaction>>,
    schema: Schema,
    comparator: Arc<RecordComparator>,
}

impl SortPlan {
    pub fn new(
        base_plan: Plan,
        tx: Arc<Mutex<Transaction>>,
        comparator: Arc<RecordComparator>,
    ) -> Self {
        let schema = base_plan.schema().clone();
        Self {
            base_plan: RefCell::new(Some(Box::new(base_plan))),
            tx,
            schema,
            comparator,
        }
    }

    fn split_into_runs(&self, mut base_scan: Scan) -> Result<Vec<TempTable>, TransactionError> {
        let mut temp_tables = vec![];
        base_scan.before_first()?;

        let mut current_temp_table = TempTable::new(self.tx.clone(), &self.schema);
        let mut current_scan = Scan::from(current_temp_table.open()?);
        temp_tables.push(current_temp_table);

        if !base_scan.next()? {
            return Ok(temp_tables);
        }

        while self.copy_and_next(&mut base_scan, &mut current_scan)? {
            if self.comparator.compare(&mut current_scan, &mut base_scan)
                == std::cmp::Ordering::Greater
            {
                current_temp_table = TempTable::new(self.tx.clone(), &self.schema);
                current_scan = Scan::from(current_temp_table.open()?);
                temp_tables.push(current_temp_table);
            }
        }
        Ok(temp_tables)
    }

    fn merge_runs(&self, mut runs: Vec<TempTable>) -> Result<Vec<TempTable>, TransactionError> {
        let mut results = vec![];
        while runs.len() > 1 {
            let p1 = runs.pop().unwrap();
            let p2 = runs.pop().unwrap();
            results.push(self.merge_two_runs(p1, p2)?);
        }
        if runs.len() == 1 {
            results.push(runs.pop().unwrap());
        }
        Ok(results)
    }

    fn merge_two_runs(&self, p1: TempTable, p2: TempTable) -> Result<TempTable, TransactionError> {
        let mut s1 = Scan::from(p1.open()?);
        let mut s2 = Scan::from(p2.open()?);
        let result = TempTable::new(self.tx.clone(), &self.schema);
        let mut result_scan = Scan::from(result.open()?);

        let mut has_next1 = s1.next()?;
        let mut has_next2 = s2.next()?;
        while has_next1 && has_next2 {
            let cmp = self.comparator.compare(&mut s1, &mut s2);
            if cmp == std::cmp::Ordering::Less {
                has_next1 = self.copy_and_next(&mut s1, &mut result_scan)?;
            } else {
                has_next2 = self.copy_and_next(&mut s2, &mut result_scan)?;
            }
        }

        while has_next1 {
            has_next1 = self.copy_and_next(&mut s1, &mut result_scan)?;
        }

        while has_next2 {
            has_next2 = self.copy_and_next(&mut s2, &mut result_scan)?;
        }
        Ok(result)
    }

    fn copy_and_next(&self, src: &mut Scan, dst: &mut Scan) -> Result<bool, TransactionError> {
        dst.insert()?;
        for field_name in self.schema().get_fields() {
            dst.set_value(&field_name, &src.get_value(&field_name)?)?;
        }
        src.next()
    }
}

impl PlanControl for SortPlan {
    fn get_num_accessed_blocks(&self) -> usize {
        let base_plan = self.base_plan.borrow_mut().take().unwrap();
        let mp = MaterializePlan::new(*base_plan, self.tx.clone());
        let num_accessed_blocks = mp.get_num_accessed_blocks();
        self.base_plan.replace(Some(mp.base_plan));
        num_accessed_blocks
    }

    fn get_num_output_records(&self) -> usize {
        self.base_plan
            .borrow()
            .as_ref()
            .unwrap()
            .get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.base_plan
            .borrow()
            .as_ref()
            .unwrap()
            .num_distinct_values(field_name)
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn open(&mut self, _tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let base_scan = self
            .base_plan
            .borrow_mut()
            .as_mut()
            .unwrap()
            .open(self.tx.clone())?;
        let mut runs: Vec<TempTable> = self.split_into_runs(base_scan)?;

        while runs.len() > 1 {
            runs = self.merge_runs(runs)?;
        }
        SortScan::new(runs, self.comparator.clone()).map(|scan| Scan::from(scan))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        materialization::record_comparator::RecordComparator,
        plan::{table_plan::TablePlan, Plan, PlanControl},
        record::schema::Schema,
        scan::ScanControl,
    };

    use super::SortPlan;

    #[test]
    fn test_sort_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        let table_name = "test_table";
        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("test_table", &schema, tx.clone())?;

        let mut base_plan = TablePlan::new(tx.clone(), table_name, db.metadata_manager.clone())?;

        {
            // Insert 20 records into the temp table
            let mut base_scan = base_plan.open(tx.clone())?;
            base_scan.before_first()?;
            for i in 0..20 {
                base_scan.insert()?;
                base_scan.set_i32("A", -i)?;
                base_scan.set_string("B", &format!("B{}", i))?;
            }
        }

        let comparator = Arc::new(RecordComparator::new(&&vec!["A".to_string()]));
        let mut sort_plan = SortPlan::new(Plan::from(base_plan), tx.clone(), comparator);
        let mut sort_scan = sort_plan.open(tx.clone())?;
        sort_scan.before_first()?;
        for i in 0..20 {
            assert!(sort_scan.next()?);
            assert_eq!(sort_scan.get_i32("A")?, i - 19);
            assert_eq!(sort_scan.get_string("B")?, format!("B{}", 19 - i));
        }

        Ok(())
    }
}
