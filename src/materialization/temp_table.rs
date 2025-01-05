use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use crate::{
    errors::TransactionError,
    record::{layout::Layout, schema::Schema},
    scan::table_scan::TableScan,
    tx::transaction::Transaction,
};

// Temporary table for storing intermediate results
pub struct TempTable {
    tx: Arc<Mutex<Transaction>>,
    table_name: String,
    layout: Arc<Layout>,
}

static TEMP_TABLE_NAME_ID: AtomicUsize = AtomicUsize::new(0);

fn get_next_table_name() -> String {
    let id = TEMP_TABLE_NAME_ID.fetch_add(1, Ordering::SeqCst);
    format!("temp_{}", id)
}

impl TempTable {
    pub fn new(tx: Arc<Mutex<Transaction>>, schema: &Schema) -> Self {
        let table_name = get_next_table_name();
        let layout = Arc::new(Layout::new(schema.clone()));
        TempTable {
            tx,
            table_name,
            layout,
        }
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_layout(&self) -> Arc<Layout> {
        self.layout.clone()
    }

    pub fn open(&self) -> Result<TableScan, TransactionError> {
        TableScan::new(self.tx.clone(), &self.table_name, self.layout.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{db::SimpleDB, errors::TransactionError, record::schema::Schema};

    use super::*;

    #[test]
    fn test_temp_table() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        {
            let temp_table = TempTable::new(tx.clone(), &schema);
            assert_eq!(temp_table.get_table_name(), "temp_0");
        }

        {
            let temp_table = TempTable::new(tx.clone(), &schema);
            assert_eq!(temp_table.get_table_name(), "temp_1");
        }

        Ok(())
    }
}
