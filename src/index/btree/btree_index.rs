use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    index::IndexControl,
    metadata::index_manager::{INDEX_BLOCK_SLOT_COLUMN, INDEX_VALUE_COLUMN},
    record::{
        field::{Spec, Value},
        layout::Layout,
        schema::Schema,
    },
    scan::RecordId,
    tx::transaction::Transaction,
};

use super::{btree_directory::BTreeDirectory, btree_leaf::BTreeLeaf, btree_page::BTreePage};

pub struct BTreeIndex {
    tx: Arc<Mutex<Transaction>>,
    leaf_layout: Layout,
    directory_layout: Layout,
    directory_root_block: BlockId,
    btree_leaf: Option<BTreeLeaf>,
    btree_leaf_file_name: String,
}

impl BTreeIndex {
    pub(crate) fn new(
        tx: Arc<Mutex<Transaction>>,
        index_name: String,
        leaf_layout: Layout,
    ) -> Result<Self, TransactionError> {
        let leaf_table = format!("{}_leaf", index_name);
        {
            if tx.lock().unwrap().get_num_blocks(&leaf_table)? == 0 {
                let block = tx.lock().unwrap().append_block(&leaf_table)?;
                let node = BTreePage::new(tx.clone(), block, leaf_layout.clone()).unwrap();
                node.format(-1)?;
            }
        }

        let mut directory_schema = Schema::new();
        directory_schema.add_field(
            INDEX_VALUE_COLUMN,
            &leaf_layout.schema.get_field_spec(INDEX_VALUE_COLUMN),
        );
        directory_schema.add_i32_field(INDEX_BLOCK_SLOT_COLUMN);
        let directory_layout = Layout::new(directory_schema);

        let directory_table = format!("{}_directory", index_name);
        let directory_root_block = BlockId::new(&directory_table, 0);
        {
            let num_blocks = tx.lock().unwrap().get_num_blocks(&directory_table)?;
            if num_blocks == 0 {
                let block = tx.lock().unwrap().append_block(&directory_table)?;
                assert!(block.block_slot == directory_root_block.block_slot);
                let node = BTreePage::new(
                    tx.clone(),
                    directory_root_block.clone(),
                    directory_layout.clone(),
                )?;
                node.format(0)?;

                let min_val = match leaf_layout.schema.get_field_spec(INDEX_VALUE_COLUMN) {
                    Spec::I32 => Value::I32(i32::MIN),
                    Spec::VarChar(_) => Value::String("".to_string()),
                };
                // insert initial directory entry
                node.insert_directory(0, min_val, 0)?;
            }
        }
        Ok(Self {
            tx,
            leaf_layout,
            directory_layout,
            directory_root_block,
            btree_leaf: None,
            btree_leaf_file_name: leaf_table,
        })
    }

    pub(crate) fn get_search_cost(num_blocks: usize, records_per_block: usize) -> usize {
        1 + (((num_blocks + 1) as f64).log2().ceil()
            / ((records_per_block + 1) as f64).log2().ceil()) as usize
    }

    #[allow(dead_code)]
    pub(crate) fn debug_print(&self) -> Result<(), TransactionError> {
        // Print tree structure of BTreeDirectory and BTreeLeaf

        let root = BTreeDirectory::new(
            self.tx.clone(),
            self.directory_root_block.clone(),
            self.directory_layout.clone(),
        )?;
        root.debug_print(&self.btree_leaf_file_name, &self.leaf_layout, 0)?;

        Ok(())
    }
}

impl IndexControl for BTreeIndex {
    fn before_first(&mut self, search_key: &Value) -> Result<(), TransactionError> {
        let mut root = BTreeDirectory::new(
            self.tx.clone(),
            self.directory_root_block.clone(),
            self.directory_layout.clone(),
        )?;

        let leaf_block_slot = root.search(search_key)?;
        let leaf_block = BlockId::new(&self.btree_leaf_file_name, leaf_block_slot);

        self.btree_leaf = Some(BTreeLeaf::new(
            self.tx.clone(),
            leaf_block,
            self.leaf_layout.clone(),
            search_key.clone(),
        )?);
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        let leaf_page = self.btree_leaf.as_mut().unwrap();
        leaf_page.next()
    }

    fn get(&self) -> Result<RecordId, TransactionError> {
        let leaf_page = self.btree_leaf.as_ref().unwrap();
        leaf_page.get_data_record_id()
    }

    fn insert(&mut self, value: &Value, record_id: &RecordId) -> Result<(), TransactionError> {
        self.before_first(value)?;
        let leaf = self.btree_leaf.as_mut().unwrap();

        match leaf.insert(record_id)? {
            Some(entry) => {
                let mut root = BTreeDirectory::new(
                    self.tx.clone(),
                    self.directory_root_block.clone(),
                    self.directory_layout.clone(),
                )?;

                match root.insert(&entry)? {
                    Some(new_root_entry) => {
                        root.make_new_root(&new_root_entry)?;
                    }
                    None => {}
                }
                // self.debug_print()?;
            }
            None => {}
        }
        Ok(())
    }

    fn delete(&mut self, value: &Value, record_id: &RecordId) -> Result<(), TransactionError> {
        self.before_first(value)?;
        let leaf = self.btree_leaf.as_mut().unwrap();
        leaf.delete(record_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{db::SimpleDB, record::schema::Schema};

    use super::*;

    const DUMMY_BLOCK_SLOT: usize = 10;

    fn test_b_tree_index_retrieval(n: usize) -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
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

        metadata_manager.lock().unwrap().create_index(
            "test_index",
            "test_table",
            "B",
            tx.clone(),
        )?;

        let mut index = metadata_manager
            .lock()
            .unwrap()
            .get_index_info("test_table", tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        let mut expected_record_ids = vec![];
        {
            for i in 0..n {
                if i % 4 == 2 {
                    expected_record_ids.push(i);
                }
                index.insert(
                    &Value::String((i % 4).to_string()),
                    &RecordId(DUMMY_BLOCK_SLOT, i),
                )?;
            }
        }

        index.before_first(&Value::String("2".to_string()))?;
        let mut actual_record_ids = vec![];
        while index.next()? {
            let record_id = index.get()?;
            assert_eq!(record_id.0, DUMMY_BLOCK_SLOT);
            actual_record_ids.push(record_id.1);
        }
        actual_record_ids.sort();
        assert_eq!(actual_record_ids, expected_record_ids);

        drop(index);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_b_tree_index_retrieval_small() -> Result<(), anyhow::Error> {
        test_b_tree_index_retrieval(30)
    }

    #[test]
    fn test_b_tree_index_retrieval_large() -> Result<(), anyhow::Error> {
        // Confirm that B-tree index insertion & retrieval still works
        // when split occurs in the leaf node
        test_b_tree_index_retrieval(300)
    }

    #[test]
    fn test_b_tree_index_delete() -> Result<(), anyhow::Error> {
        let n = 300;
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
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

        metadata_manager.lock().unwrap().create_index(
            "test_index",
            "test_table",
            "B",
            tx.clone(),
        )?;

        let mut index = metadata_manager
            .lock()
            .unwrap()
            .get_index_info("test_table", tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        let mut expected_record_ids = vec![];
        let mut deletion_ids = vec![];

        for i in 0..n {
            if i % 4 == 2 {
                if i % 3 == 2 {
                    expected_record_ids.push(i);
                } else {
                    deletion_ids.push(i);
                }
            }
            index.insert(
                &Value::String((i % 4).to_string()),
                &RecordId(DUMMY_BLOCK_SLOT, i),
            )?;
        }

        for i in deletion_ids {
            index.delete(
                &Value::String((i % 4).to_string()),
                &RecordId(DUMMY_BLOCK_SLOT, i),
            )?;
        }

        index.before_first(&Value::String("2".to_string()))?;
        let mut actual_record_ids = vec![];
        while index.next()? {
            let record_id = index.get()?;
            assert_eq!(record_id.0, DUMMY_BLOCK_SLOT);
            actual_record_ids.push(record_id.1);
        }
        actual_record_ids.sort();
        assert_eq!(actual_record_ids, expected_record_ids);

        drop(index);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_b_tree_index_null() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
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

        metadata_manager.lock().unwrap().create_index(
            "test_index",
            "test_table",
            "B",
            tx.clone(),
        )?;

        let mut index = metadata_manager
            .lock()
            .unwrap()
            .get_index_info("test_table", tx.clone())?
            .get("B")
            .unwrap()
            .open()?;

        index.insert(&Value::String("1".to_string()), &RecordId(DUMMY_BLOCK_SLOT, 0))?;
        index.insert(&Value::Null, &RecordId(DUMMY_BLOCK_SLOT, 1))?;
        index.insert(&Value::String("2".to_string()), &RecordId(DUMMY_BLOCK_SLOT, 2))?;

        index.before_first(&Value::Null)?;
        assert!(index.next()?);
        let rid = index.get()?;
        assert_eq!(rid.0, DUMMY_BLOCK_SLOT);
        assert_eq!(rid.1, 1);
        assert!(!index.next()?);

        drop(index);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
