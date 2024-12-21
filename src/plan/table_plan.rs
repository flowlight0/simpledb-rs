use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    metadata::{stat_manager::StatInfo, MetadataManager},
    record::{layout::Layout, schema::Schema},
    scan::{table_scan::TableScan, Scan},
    tx::transaction::Transaction,
};

use super::Plan;

pub struct TablePlan {
    table_name: String,
    layout: Arc<Layout>,
    stat_info: StatInfo,
}

impl TablePlan {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        table_name: &str,
        metadata_manager: Arc<Mutex<MetadataManager>>,
    ) -> Result<Self, TransactionError> {
        let mut metadata_manager = metadata_manager.lock().unwrap();
        let layout = Arc::new(
            metadata_manager
                .get_layout(table_name, tx.clone())?
                .unwrap(),
        );
        let stat_info = metadata_manager.get_stat_info(table_name, layout.clone(), tx)?;
        Ok(TablePlan {
            table_name: table_name.to_string(),
            layout,
            stat_info,
        })
    }
}

impl Plan for TablePlan {
    fn get_num_accessed_blocks(&self) -> usize {
        self.stat_info.get_num_blocks()
    }

    fn get_num_output_records(&self) -> usize {
        self.stat_info.get_num_records()
    }

    fn num_distinct_values(&self, field_name: &str) -> usize {
        self.stat_info.get_distinct_values(field_name)
    }

    fn schema(&self) -> &Schema {
        &self.layout.schema
    }

    fn open(&mut self, tx: Arc<Mutex<Transaction>>) -> Result<Box<dyn Scan>, TransactionError> {
        let table_scan = TableScan::new(tx, &self.table_name, self.layout.clone())?;
        Ok(Box::new(table_scan))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SimpleDB;
    use crate::record::layout::Layout;
    use crate::record::schema::Schema;

    #[test]
    fn test_table_plan() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 3;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let table_name = "testtable";
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut metadata_manager = db.metadata_manager.lock().unwrap();
        metadata_manager.create_table(table_name, &layout.schema, tx.clone())?;

        let mut table_scan = TableScan::new(tx.clone(), table_name, layout.clone())?;
        table_scan.before_first()?;
        for i in 0..200 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }
        drop(table_scan);
        tx.lock().unwrap().commit()?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        metadata_manager
            .stat_manager
            .lock()
            .unwrap()
            .refresh_statistics(tx.clone())?;
        drop(metadata_manager);
        let table_plan = TablePlan::new(tx.clone(), table_name, db.metadata_manager.clone())?;
        assert!(table_plan.get_num_accessed_blocks() > 0);
        assert_eq!(table_plan.get_num_output_records(), 200);
        assert!(table_plan.num_distinct_values("A") > 0);
        Ok(())
    }
}
