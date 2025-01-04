use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    index::btree::btree_leaf::BTreeLeaf,
    record::{field::Value, layout::Layout},
    tx::transaction::Transaction,
};

use super::{btree_page::BTreePage, DirectoryEntry};

pub struct BTreeDirectory {
    tx: Arc<Mutex<Transaction>>,
    layout: Layout,
    pub contents: BTreePage,
    file_name: String,
}

impl BTreeDirectory {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        block: BlockId,
        layout: Layout,
    ) -> Result<Self, TransactionError> {
        let file_name = block.file_name.clone();
        let contents = BTreePage::new(tx.clone(), block, layout.clone())?;
        Ok(Self {
            tx,
            layout,
            contents,
            file_name,
        })
    }

    pub(crate) fn search(&mut self, search_key: &Value) -> Result<usize, TransactionError> {
        let mut current_contents = BTreePage::new(
            self.tx.clone(),
            self.contents.block.clone(),
            self.layout.clone(),
        )?;
        while current_contents.get_flag()? > 0 {
            let child_block_slot = self.find_child_block_slot(search_key)?;
            let child_block = BlockId {
                file_name: self.file_name.clone(),
                block_slot: child_block_slot,
            };
            current_contents =
                BTreePage::new(self.tx.clone(), child_block.clone(), self.layout.clone())?;
        }

        assert_eq!(current_contents.get_flag()?, 0);
        self.find_child_block_slot(search_key)
    }

    pub(crate) fn make_new_root(
        &mut self,
        new_entry: &DirectoryEntry,
    ) -> Result<(), TransactionError> {
        let first_value = self.contents.get_data_value(0)?;
        let level = self.contents.get_flag()?;

        // transfer all the contents of the current root to a new block
        let new_block = self.contents.split(0, level)?;
        let old_entry = DirectoryEntry {
            data_value: first_value,
            block_slot: new_block.block_slot,
        };
        self.insert_entry(&old_entry)?;
        self.insert_entry(new_entry)?;
        self.contents.set_flag(level + 1)
    }

    pub(crate) fn insert(
        &mut self,
        directory_entry: &DirectoryEntry,
    ) -> Result<Option<DirectoryEntry>, TransactionError> {
        if self.contents.get_flag()? == 0 {
            self.insert_entry(directory_entry)
        } else {
            let child_block_slot = self.find_child_block_slot(&directory_entry.data_value)?;
            let mut child_btree_directory = BTreeDirectory::new(
                self.tx.clone(),
                BlockId {
                    file_name: self.file_name.clone(),
                    block_slot: child_block_slot,
                },
                self.layout.clone(),
            )?;
            let returned_entry = child_btree_directory.insert(directory_entry)?;
            if let Some(entry) = returned_entry {
                self.insert_entry(&entry)
            } else {
                Ok(None)
            }
        }
    }

    fn insert_entry(
        &mut self,
        directory_entry: &DirectoryEntry,
    ) -> Result<Option<DirectoryEntry>, TransactionError> {
        let slot = self
            .contents
            .find_slot_before(&directory_entry.data_value)?;
        let new_slot_index = slot.index() + 1;
        self.contents.insert_directory(
            new_slot_index,
            directory_entry.data_value.clone(),
            directory_entry.block_slot,
        )?;
        if self.contents.is_full()? {
            let level = self.contents.get_flag()?;
            let split_position = self.contents.get_num_records()? / 2;
            let split_key = self.contents.get_data_value(split_position)?;
            let new_block = self.contents.split(split_position, level)?;
            Ok(Some(DirectoryEntry {
                data_value: split_key,
                block_slot: new_block.block_slot,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_child_block_slot(&self, search_key: &Value) -> Result<usize, TransactionError> {
        let slot = self.contents.find_slot_before(search_key)?;
        let mut slot_index = slot.index();

        if self.contents.get_data_value(slot_index + 1)? == *search_key {
            slot_index += 1;
        }
        self.contents.get_child_block_slot(slot_index)
    }

    #[allow(dead_code)]
    pub(crate) fn debug_print(
        &self,
        btree_leaf_file_name: &str,
        btree_leaf_layout: &Layout,
        depth: usize,
    ) -> Result<(), TransactionError> {
        let level = self.contents.get_flag()?;

        let spaces = " ".repeat(depth as usize * 2);
        for slot in 0..self.contents.get_num_records()? {
            let data_value = self.contents.get_data_value(slot)?;
            let block_slot = self.contents.get_child_block_slot(slot)?;
            eprintln!("{}{}: {:?} {}", spaces, level, data_value, block_slot);

            if level > 0 {
                let child_block = BlockId {
                    file_name: self.file_name.clone(),
                    block_slot,
                };
                let child_btree_directory =
                    BTreeDirectory::new(self.tx.clone(), child_block, self.layout.clone())?;
                child_btree_directory.debug_print(
                    btree_leaf_file_name,
                    btree_leaf_layout,
                    depth + 1,
                )?;
            } else {
                let child_block = BlockId {
                    file_name: btree_leaf_file_name.to_string(),
                    block_slot,
                };

                let child_btree_leaf = BTreeLeaf::new(
                    self.tx.clone(),
                    child_block,
                    btree_leaf_layout.clone(),
                    Value::String("3".to_owned()), // dummy
                )?;
                child_btree_leaf.debug_print(depth + 1)?;
            }
        }
        Ok(())
    }
}
