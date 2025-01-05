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

    pub(crate) fn get_tx(&self) -> Arc<Mutex<Transaction>> {
        self.tx.clone()
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
