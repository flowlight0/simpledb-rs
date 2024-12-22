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
        record::schema::Schema,
        scan::{table_scan::TableScan, Scan},
    };

    use super::*;

    #[test]
    fn test_b_tree_index_retrieval() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 4096;
        let num_buffers = 256;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let metadata_manager = db.metadata_manager.clone();

        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);

        metadata_manager
            .lock()
            .unwrap()
            .create_table("test_table", &schema, tx.clone())?;

        let layout = Arc::new(Layout::new(schema));

        let mut table_scan = TableScan::new(tx.clone(), "test_table", layout)?;
        for i in 0..1 {
            table_scan.insert()?;
            table_scan.set_i32("A", i % 3)?;
            table_scan.set_string("B", &(i % 4).to_string())?;
        }
        table_scan.close()?;

        metadata_manager.lock().unwrap().create_index(
            "test_index",
            "test_table",
            "B",
            tx.clone(),
        )?;

        let mut index = metadata_manager
            .lock()
            .unwrap()
            .get_index_info("test_index", tx.clone())?
            .get("test_index")
            .unwrap()
            .open();
        index.before_first(Value::String("2".to_string()))?;
        while index.next()? {
            let record_id = index.get()?;
            table_scan.move_to_record_id(record_id)?;
            assert_eq!(table_scan.get_string("B")?, "2".to_string());
            table_scan.close()?;
        }

        index.close()?;
        table_scan.close()?;
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}