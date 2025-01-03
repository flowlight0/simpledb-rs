use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    record::{field::Value, layout::Layout, record_page::Slot},
    scan::RecordId,
    tx::transaction::Transaction,
};

use super::{btree_page::BTreePage, DirectoryEntry};

pub struct BTreeLeaf {
    pub contents: BTreePage,
    current_slot: Slot,
    search_key: Value,
}

impl BTreeLeaf {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        block: BlockId,
        layout: Layout,
        search_key: Value,
    ) -> Result<Self, TransactionError> {
        let contents = BTreePage::new(tx, block, layout)?;
        let current_slot = contents.find_slot_before(&search_key)?;
        Ok(Self {
            contents,
            current_slot,
            search_key,
        })
    }

    pub fn next(&mut self) -> Result<bool, TransactionError> {
        self.current_slot = self.current_slot.next();
        match self.current_slot {
            Slot::Index(index) => {
                let has_next = if index >= self.contents.get_num_records()? {
                    self.try_overflow()
                } else if self.contents.get_data_value(index)? == self.search_key {
                    Ok(true)
                } else {
                    self.try_overflow()
                }?;
                if !has_next {
                    self.current_slot = Slot::End;
                }
                Ok(has_next)
            }
            Slot::End => Ok(false),
            Slot::Start => unreachable!(),
        }
    }

    pub(crate) fn get_data_record_id(&self) -> Result<RecordId, TransactionError> {
        self.contents
            .get_data_record_id(self.current_slot.get_index())
    }

    pub fn delete(&mut self, record_id: &RecordId) -> Result<(), TransactionError> {
        while self.next()? {
            if self.get_data_record_id()? == *record_id {
                self.contents.delete(self.current_slot.get_index())?;
                return Ok(());
            }
        }
        Ok(())
    }

    pub(crate) fn insert(
        &mut self,
        record_id: &RecordId,
    ) -> Result<Option<DirectoryEntry>, TransactionError> {
        let overflow_pointer = self.contents.get_flag()?;

        let first_value = self.contents.get_data_value(0)?;
        if overflow_pointer >= 0 && first_value > self.search_key {
            unreachable!();
            let new_block = self.contents.split(0, overflow_pointer)?;

            self.current_slot = Slot::Start;
            self.contents.set_flag(-1)?;
            self.contents.insert_leaf(0, &self.search_key, record_id)?;
            return Ok(Some(DirectoryEntry {
                data_value: first_value,
                block_slot: new_block.block_slot,
            }));
        }

        self.current_slot = self.current_slot.next();
        self.contents
            .insert_leaf(self.current_slot.get_index(), &self.search_key, record_id)?;

        if !self.contents.is_full()? {
            return Ok(None);
        }

        // split the leaf block
        let first_key = self.contents.get_data_value(0)?;
        let last_key = self
            .contents
            .get_data_value(self.contents.get_num_records()? - 1)?;

        if first_key == last_key {
            // Create an overflow block to hold all the but the first record
            eprintln!("Leaf became full, creating overflow block");
            let new_block = self.contents.split(1, self.contents.get_flag()?)?;
            eprintln!("Overflow block = {:?}", new_block.block_slot);
            self.contents.set_flag(new_block.block_slot as i32)?;
            Ok(None)
        } else {
            let mut split_pos = self.contents.get_num_records()? / 2;
            let mut split_key = self.contents.get_data_value(split_pos)?;

            // Move split position such that the same key is not in both blocks
            if split_key == first_key {
                // move right to find the split position
                while self.contents.get_data_value(split_pos)? == split_key {
                    split_pos += 1;
                }
                split_key = self.contents.get_data_value(split_pos)?;
            } else {
                // move left to find the split position
                while self.contents.get_data_value(split_pos - 1)? == split_key {
                    split_pos -= 1;
                }
            }
            let new_block = self.contents.split(split_pos, -1)?;
            Ok(Some(DirectoryEntry {
                data_value: split_key,
                block_slot: new_block.block_slot,
            }))
        }
    }

    fn try_overflow(&mut self) -> Result<bool, TransactionError> {
        let first_key = self.contents.get_data_value(0)?;
        let overflow_pointer = self.contents.get_flag()?;
        if first_key != self.search_key || overflow_pointer < 0 {
            Ok(false)
        } else {
            let next_block =
                BlockId::new(&self.contents.block.file_name, overflow_pointer as usize);
            self.contents = BTreePage::new(
                self.contents.tx.clone(),
                next_block,
                self.contents.layout.clone(),
            )?;
            self.current_slot = Slot::Start;
            self.next()
        }
    }

    #[allow(dead_code)]
    pub(crate) fn debug_print(&self, depth: usize) -> Result<(), TransactionError> {
        let spaces = " ".repeat(depth * 2);
        for i in 0..self.contents.get_num_records()? {
            let data_value = self.contents.get_data_value(i)?;
            let record_id = self.contents.get_data_record_id(i)?;
            eprintln!("{}LEAF[{}]: {:?} {:?}", spaces, i, data_value, record_id);
        }

        Ok(())
    }
}
