use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use index_manager::{IndexInfo, IndexManager};
use stat_manager::{StatInfo, StatManager};
use table_manager::TableManager;

use crate::{
    errors::TransactionError,
    record::{layout::Layout, schema::Schema},
    tx::transaction::Transaction,
};

pub mod index_manager;
pub mod stat_manager;
pub mod table_manager;

pub struct MetadataManager {
    table_manager: Arc<Mutex<TableManager>>,
    pub(crate) stat_manager: Arc<Mutex<StatManager>>,
    index_manager: Arc<Mutex<IndexManager>>,
}

impl MetadataManager {
    pub fn new(is_new: bool, tx: Arc<Mutex<Transaction>>) -> Result<Self, TransactionError> {
        let table_manager = Arc::new(Mutex::new(TableManager::new(is_new, tx.clone())?));
        let stat_manager = Arc::new(Mutex::new(StatManager::new(table_manager.clone())?));
        let index_manager = Arc::new(Mutex::new(IndexManager::new(
            is_new,
            table_manager.clone(),
            stat_manager.clone(),
            tx.clone(),
        )));
        Ok(Self {
            table_manager,
            index_manager,
            stat_manager,
        })
    }

    pub fn create_table(
        &mut self,
        table_name: &str,
        schema: &Schema,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<(), TransactionError> {
        let table_manager = self.table_manager.lock().unwrap();
        table_manager.create_table(table_name, schema, tx)
    }

    pub fn get_layout(
        &self,
        table_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<Option<Layout>, TransactionError> {
        let table_manager = self.table_manager.lock().unwrap();
        table_manager.get_layout(table_name, tx)
    }

    pub fn create_index(
        &self,
        index_name: &str,
        table_name: &str,
        field_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<(), TransactionError> {
        let index_manager = self.index_manager.lock().unwrap();
        index_manager.create_index(index_name, table_name, field_name, tx)
    }

    pub fn get_index_info(
        &self,
        table_name: &str,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<HashMap<String, IndexInfo>, TransactionError> {
        let index_manager = self.index_manager.lock().unwrap();
        index_manager.get_index_info(table_name, tx)
    }

    pub fn get_stat_info(
        &mut self,
        table_name: &str,
        layout: Arc<Layout>,
        tx: Arc<Mutex<Transaction>>,
    ) -> Result<StatInfo, TransactionError> {
        // self.stat_manager.get_stat_info(table_name, layout, tx)
        let mut stat_manager = self.stat_manager.lock().unwrap();
        stat_manager.get_stat_info(table_name, layout, tx)
    }
}

#[cfg(test)]
mod tests {

    use std::sync::{Arc, Mutex};

    use crate::db::SimpleDB;

    use crate::errors::TransactionError;
    use crate::record::schema::Schema;
    use crate::scan::table_scan::TableScan;
    use crate::scan::Scan;

    #[test]
    fn test_metadata_manager() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 3;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        db.metadata_manager
            .lock()
            .unwrap()
            .create_table("testtable", &schema, tx.clone())?;
        let layout = Arc::new(
            db.metadata_manager
                .lock()
                .unwrap()
                .get_layout("testtable", tx.clone())?
                .unwrap(),
        );
        assert_eq!(&layout.schema, &schema);

        let mut table_scan = TableScan::new(tx.clone(), "testtable", layout.clone())?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;
        }
        drop(table_scan);

        let stat_info = db.metadata_manager.lock().unwrap().get_stat_info(
            "testtable",
            layout.clone(),
            tx.clone(),
        )?;
        assert_eq!(stat_info.get_num_records(), 50);
        // It's obvious that the number of blocks is greater than 1
        assert!(stat_info.get_num_blocks() > 1);
        Ok(())
    }
}
