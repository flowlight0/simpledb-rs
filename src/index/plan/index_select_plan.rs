use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    index::scan::index_select_scan::IndexSelectScan,
    metadata::index_manager::IndexInfo,
    plan::{table_plan::TablePlan, PlanControl},
    record::{field::Value, schema::Schema},
    scan::Scan,
    tx::transaction::Transaction,
};

#[derive(Clone)]
pub struct IndexSelectPlan {
    table_plan: TablePlan,
    index_info: IndexInfo,
    value: Value,
}

impl IndexSelectPlan {
    pub fn new(table_plan: TablePlan, index_info: &IndexInfo, value: &Value) -> Self {
        IndexSelectPlan {
            table_plan,
            index_info: index_info.clone(),
            value: value.clone(),
        }
    }
}

impl PlanControl for IndexSelectPlan {
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

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Scan, TransactionError> {
        let scan = self.table_plan.open(tx.clone())?;
        let value = self.value.clone();
        match scan {
            Scan::TableScan(table_scan) => Ok(Scan::from(IndexSelectScan::new(
                table_scan,
                self.index_info.open()?,
                value,
            )?)),
            _ => panic!("Expected TableScan"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexControl;
    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
        scan::{table_scan::TableScan, RecordId, Scan, ScanControl},
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_index_select_plan_open_and_stats() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let num_buffers = 256;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let table_name = "test_table";
        let index_name = "test_index";
        {
            let mut mm = db.metadata_manager.lock().unwrap();
            mm.create_table(table_name, &layout.schema, tx.clone())?;
            mm.create_index(index_name, table_name, "B", tx.clone())?;
        }

        let mut table_scan = TableScan::new(tx.clone(), table_name, layout.clone())?;
        let mut index = db
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        let mut expected = Vec::new();
        table_scan.before_first()?;
        for i in 0..30 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            let val = (i % 4).to_string();
            table_scan.set_string("B", &val)?;
            index.insert(
                &Value::String(val.clone()),
                &RecordId::from(table_scan.get_record_pointer()),
            )?;
            if val == "2" {
                expected.push(i);
            }
        }
        drop(index);
        drop(table_scan);
        tx.lock().unwrap().commit()?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        db.metadata_manager
            .lock()
            .unwrap()
            .stat_manager
            .lock()
            .unwrap()
            .refresh_statistics(tx.clone())?;

        let table_plan = TablePlan::new(tx.clone(), table_name, db.metadata_manager.clone())?;
        let index_info = db
            .metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?
            .get("B")
            .unwrap()
            .clone();

        let mut plan =
            IndexSelectPlan::new(table_plan, &index_info, &Value::String("2".to_string()));
        let mut scan = match plan.open(tx.clone())? {
            Scan::IndexSelectScan(s) => s,
            _ => panic!("Expected IndexSelectScan"),
        };

        scan.before_first()?;
        let mut actual = Vec::new();
        while scan.next()? {
            actual.push(scan.get_i32("A")?.unwrap());
        }
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);

        assert!(plan.get_num_accessed_blocks() > 0);
        assert!(plan.get_num_output_records() > 0);
        assert!(plan.num_distinct_values("B") >= 1);
        assert!(plan.schema().has_field("A"));

        Ok(())
    }
}
