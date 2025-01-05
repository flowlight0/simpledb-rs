use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    plan::{Plan, PlanControl},
    record::{layout::Layout, schema::Schema},
    scan::{Scan, ScanControl},
    tx::transaction::Transaction,
};

use super::temp_table::TempTable;

pub struct MaterializePlan {
    pub(crate) base_plan: Box<Plan>,
    tx: Arc<Mutex<Transaction>>,
}

impl MaterializePlan {
    pub fn new(base_plan: Plan, tx: Arc<Mutex<Transaction>>) -> Self {
        MaterializePlan {
            base_plan: Box::new(base_plan),
            tx,
        }
    }
}

impl PlanControl for MaterializePlan {
    fn get_num_accessed_blocks(&self) -> usize {
        let dummy_layout = Layout::new(self.base_plan.schema().clone());
        let slot_size = dummy_layout.slot_size;
        let record_per_block = self.tx.lock().unwrap().get_block_size() / slot_size;
        let num_output_records = self.get_num_output_records();
        (num_output_records + record_per_block - 1) / record_per_block
    }

    fn get_num_output_records(&self) -> usize {
        self.base_plan.get_num_output_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.base_plan.num_distinct_values(field_name)
    }

    fn schema(&self) -> &Schema {
        self.base_plan.schema()
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let temp_table = TempTable::new(tx.clone(), self.schema());
        let mut base_scan = self.base_plan.open(tx.clone())?;
        let mut temp_table_scan = temp_table.open()?;
        while base_scan.next()? {
            temp_table_scan.insert()?;
            for field_name in self.schema().get_fields() {
                let value = base_scan.get_value(&field_name)?;
                temp_table_scan.set_value(&field_name, &value)?;
            }
        }
        temp_table_scan.before_first()?;
        Ok(Scan::from(temp_table_scan))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        plan::table_plan::TablePlan,
        record::{layout::Layout, schema::Schema},
        scan::table_scan::TableScan,
    };

    use super::*;

    #[test]
    fn test_materialize_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 256;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        // Create a table with two fields
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("T", &schema, tx.clone())?;

        // Insert 20 dummy records into the table
        let mut table_scan = TableScan::new(tx.clone(), "T", Arc::new(Layout::new(schema)))?;
        table_scan.before_first()?;
        for i in 0..20 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }
        drop(table_scan);

        // Materialize the table
        let base_plan = Plan::from(TablePlan::new(
            tx.clone(),
            "T",
            db.metadata_manager.clone(),
        )?);
        let mut plan = MaterializePlan::new(base_plan, tx.clone());
        let mut materialized_scan = plan.open(tx.clone())?;

        // Check that the materialized table has the same records as the original table
        // even when we scan it multiple times
        for _ in 0..2 {
            let mut count = 0;
            materialized_scan.before_first()?;
            while materialized_scan.next()? {
                assert_eq!(materialized_scan.get_i32("A")?, count);
                assert_eq!(materialized_scan.get_string("B")?, count.to_string());
                count += 1;
            }
            assert_eq!(count, 20);
        }
        Ok(())
    }
}
