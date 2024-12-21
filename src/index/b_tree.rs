use std::sync::{Arc, Mutex};

use crate::{
    record::{field::Value, layout::Layout},
    tx::transaction::Transaction,
};

use super::{IndexControl, RecordId};

pub struct BTreeIndex {}
impl BTreeIndex {
    pub(crate) fn new(
        tx: Arc<Mutex<Transaction>>,
        index_name: String,
        index_layout: Layout,
    ) -> Self {
        todo!()
    }

    pub(crate) fn get_search_cost(num_blocks: usize, records_per_block: usize) -> usize {
        1
    }
}

impl IndexControl for BTreeIndex {
    fn before_first(&mut self, search_key: Value) -> Result<(), anyhow::Error> {
        todo!()
    }

    fn next(&mut self) -> Result<bool, anyhow::Error> {
        todo!()
    }

    fn get(&self) -> Result<RecordId, anyhow::Error> {
        todo!()
    }

    fn insert(&mut self, value: Value, record_id: RecordId) -> Result<(), anyhow::Error> {
        todo!()
    }

    fn delete(&mut self, value: Value, record_id: RecordId) -> Result<(), anyhow::Error> {
        todo!()
    }

    fn close(&mut self) -> Result<(), anyhow::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Ok;

    use crate::{
        db::SimpleDB,
        driver::network::metadata,
        plan::{table_plan::TablePlan, Plan},
        tx,
    };

    use super::*;

    #[test]
    fn test_b_tree_index_retrieval() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let num_buffers = 100;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let metadata_manager = db.metadata_manager.clone();

        // let mut plan = TablePlan::new(tx.clone(), "test_table", db.metadata_manager.clone())?;
        // let scan = plan.open(tx.clone())?;

        // db.metadata_manager
        //     .lock()
        //     .unwrap()
        //     .create_index("test_table", "A", "BTree", tx.clone())?;
        Ok(())
    }
}
