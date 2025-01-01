use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    metadata::index_manager::{
        INDEX_BLOCK_SLOT_COLUMN, INDEX_RECORD_SLOT_COLUMN, INDEX_VALUE_COLUMN,
    },
    record::{
        field::{Type, Value},
        layout::Layout,
        record_page::Slot,
    },
    scan::RecordId,
    tx::transaction::Transaction,
};

pub(crate) struct BTreePage {
    pub(crate) tx: Arc<Mutex<Transaction>>,
    pub(crate) block: BlockId,
    pub(crate) layout: Layout,
}

pub type LevelOrOverflowPointer = i32;

const NUM_RECORDS_OFFSET: usize = std::mem::size_of::<i32>();
const RECORD_OFFSET: usize = 2 * std::mem::size_of::<i32>();

impl BTreePage {
    pub(crate) fn new(
        tx: Arc<Mutex<Transaction>>,
        block: BlockId,
        layout: Layout,
    ) -> Result<Self, TransactionError> {
        tx.lock().unwrap().pin(&block)?;
        Ok(BTreePage { tx, block, layout })
    }

    // For a given key, find the smallest slot number in the current block that is greater than or equal to the key.
    // It then returns the slot number immediately before that slot.
    // It means that, if the returned slot index is i, self.get_data_value(i)? < key <= self.get_data_value(i + 1)?.
    //
    // This method assumes "self.get_data_value(0)? <= key" always holds true.
    pub(crate) fn find_slot_before(&self, key: &Value) -> Result<Slot, TransactionError> {
        assert!(self.get_data_value(0)? <= *key);

        let mut slot = Slot::Index(0);
        while slot.index() < self.get_num_records()? {
            let value = self.get_data_value(slot.get_index())?;
            match (value, &key) {
                (Value::I32(value), Value::I32(key)) => {
                    if value >= *key {
                        return Ok(slot.prev());
                    }
                }
                (Value::String(value), Value::String(key)) => {
                    if value >= *key {
                        return Ok(slot.prev());
                    }
                }
                _ => panic!("Incompatible types"),
            }
            slot = slot.next();
        }
        Ok(slot.prev())
    }

    pub(crate) fn is_full(&self) -> Result<bool, TransactionError> {
        let num_records = self.get_num_records()?;
        let block_size = self.tx.lock().unwrap().get_block_size();
        Ok(self.get_slot_position(num_records + 1) >= block_size)
    }

    pub(crate) fn split(
        &self,
        split_position: usize,
        flag: LevelOrOverflowPointer,
    ) -> Result<BlockId, TransactionError> {
        let new_block = self
            .tx
            .lock()
            .unwrap()
            .append_block(&self.block.file_name)?;

        let mut new_btree_page =
            BTreePage::new(self.tx.clone(), new_block.clone(), self.layout.clone())?;
        new_btree_page.format(flag)?;
        self.transfer_records(split_position, &mut new_btree_page)?;

        Ok(new_block)
    }

    pub(crate) fn get_data_value(&self, slot: usize) -> Result<Value, TransactionError> {
        self.get_value(slot, INDEX_VALUE_COLUMN)
    }

    pub(crate) fn get_flag(&self) -> Result<LevelOrOverflowPointer, TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        lock.get_i32(&self.block, 0)
    }

    pub(crate) fn set_flag(&self, flag: LevelOrOverflowPointer) -> Result<(), TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        lock.set_i32(&self.block, 0, flag, true).map(|_| ())
    }

    pub(crate) fn format(&self, flag: LevelOrOverflowPointer) -> Result<(), TransactionError> {
        self.set_flag(flag)?;
        self.set_num_records(0)?;

        let block_size = self.tx.lock().unwrap().get_block_size();
        let slot_size = self.layout.slot_size;
        let mut slot = 0;
        while RECORD_OFFSET + ((slot + 1) * slot_size) < block_size {
            for field in self.layout.schema.get_fields() {
                match self.layout.schema.get_field_type(&field) {
                    Type::I32 => {
                        self.set_i32(slot, &field, 0)?;
                    }
                    Type::String => {
                        self.set_string(slot, &field, "")?;
                    }
                }
            }
            slot += 1;
        }
        Ok(())
    }

    // Method called only by BTreeDirectory
    pub(crate) fn get_child_block_slot(&self, slot: usize) -> Result<usize, TransactionError> {
        self.get_i32(slot, INDEX_BLOCK_SLOT_COLUMN)
            .map(|i| i as usize)
    }

    // Method called only by BTreeDirectory
    pub(crate) fn insert_directory(
        &self,
        slot: usize,
        value: Value,
        block_slot: usize,
    ) -> Result<(), TransactionError> {
        self.insert_empty_slot(slot)?;
        self.set_value(slot, INDEX_VALUE_COLUMN, &value)?;
        self.set_i32(slot, INDEX_BLOCK_SLOT_COLUMN, block_slot as i32)
    }

    // Method called only by BTreeLeaf
    pub(crate) fn get_data_record_id(&self, slot: usize) -> Result<RecordId, TransactionError> {
        let block_slot = self.get_i32(slot, INDEX_BLOCK_SLOT_COLUMN)?;
        let record_slot = self.get_i32(slot, INDEX_RECORD_SLOT_COLUMN)?;
        Ok(RecordId(block_slot as usize, record_slot as usize))
    }

    // Method called only by BTreeLeaf
    pub(crate) fn insert_leaf(
        &self,
        slot: usize,
        value: &Value,
        record_id: &RecordId,
    ) -> Result<(), TransactionError> {
        self.insert_empty_slot(slot)?;
        self.set_value(slot, INDEX_VALUE_COLUMN, value)?;
        self.set_i32(slot, INDEX_BLOCK_SLOT_COLUMN, record_id.0 as i32)?;
        self.set_i32(slot, INDEX_RECORD_SLOT_COLUMN, record_id.1 as i32)
    }

    pub(crate) fn delete(&self, slot: usize) -> Result<(), TransactionError> {
        let num_records = self.get_num_records()?;
        assert!(slot < num_records);
        for i in slot + 1..num_records {
            self.copy_record(i, i - 1)?;
        }
        self.set_num_records(num_records as i32 - 1)
    }

    pub(crate) fn get_num_records(&self) -> Result<usize, TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        lock.get_i32(&self.block, NUM_RECORDS_OFFSET)
            .map(|num_records| num_records as usize)
    }

    // Private methods
    fn get_i32(&self, slot: usize, field_name: &str) -> Result<i32, TransactionError> {
        let position = self.get_field_position(slot, field_name);
        let mut lock = self.tx.lock().unwrap();
        lock.get_i32(&self.block, position)
    }

    fn get_string(&self, slot: usize, field_name: &str) -> Result<String, TransactionError> {
        let position = self.get_field_position(slot, field_name);
        let mut lock = self.tx.lock().unwrap();
        lock.get_string(&self.block, position)
    }

    fn get_value(&self, slot: usize, field_name: &str) -> Result<Value, TransactionError> {
        match self.layout.schema.get_field_type(field_name) {
            Type::I32 => {
                let value = self.get_i32(slot, field_name)?;
                Ok(Value::I32(value))
            }
            Type::String => {
                let value = self.get_string(slot, field_name)?;
                Ok(Value::String(value))
            }
        }
    }

    fn set_i32(&self, slot: usize, field_name: &str, value: i32) -> Result<(), TransactionError> {
        let position = self.get_field_position(slot, field_name);
        let mut lock = self.tx.lock().unwrap();
        lock.set_i32(&self.block, position, value, true).map(|_| ())
    }

    fn set_string(
        &self,
        slot: usize,
        field_name: &str,
        value: &str,
    ) -> Result<(), TransactionError> {
        let position = self.get_field_position(slot, field_name);
        let mut lock = self.tx.lock().unwrap();
        lock.set_string(&self.block, position, value, true)
            .map(|_| ())
    }

    fn set_value(
        &self,
        slot: usize,
        field_name: &str,
        value: &Value,
    ) -> Result<(), TransactionError> {
        match value {
            Value::I32(i) => self.set_i32(slot, field_name, *i),
            Value::String(s) => self.set_string(slot, field_name, s),
        }
    }

    fn set_num_records(&self, num_records: i32) -> Result<(), TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        lock.set_i32(&self.block, std::mem::size_of::<i32>(), num_records, true)
            .map(|_| ())
    }

    fn insert_empty_slot(&self, slot: usize) -> Result<(), TransactionError> {
        let num_records = self.get_num_records()?;
        assert!(slot <= num_records);
        for i in (slot..num_records).rev() {
            self.copy_record(i, i + 1)?;
        }
        self.set_num_records(num_records as i32 + 1)
    }

    fn copy_record(&self, from_slot: usize, to_slot: usize) -> Result<(), TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        for field in self.layout.schema.get_fields() {
            let from_field_position = self.get_field_position(from_slot, &field);
            let to_field_position = self.get_field_position(to_slot, &field);

            match self.layout.schema.get_field_type(&field) {
                Type::I32 => {
                    let value = lock.get_i32(&self.block, from_field_position)?;
                    lock.set_i32(&self.block, to_field_position, value, true)?;
                }
                Type::String => {
                    let value = lock.get_string(&self.block, from_field_position)?;
                    lock.set_string(&self.block, to_field_position, &value, true)?;
                }
            }
        }
        Ok(())
    }

    fn transfer_records(
        &self,
        slot: usize,
        new_btree_page: &mut BTreePage,
    ) -> Result<(), TransactionError> {
        let mut dest_slot = 0;
        let schema = self.layout.schema.clone();
        while slot < self.get_num_records()? {
            new_btree_page.insert_empty_slot(dest_slot)?;
            for field in schema.get_fields() {
                let value = self.get_value(slot, &field)?;
                new_btree_page.set_value(dest_slot, &field, &value)?;
            }
            self.delete(slot)?;
            dest_slot += 1;
        }
        Ok(())
    }

    fn get_field_position(&self, slot: usize, field_name: &str) -> usize {
        let slot_position = self.get_slot_position(slot);
        let field_position = self.layout.get_offset(field_name);
        slot_position + field_position
    }

    fn get_slot_position(&self, slot: usize) -> usize {
        let slot_size = self.layout.slot_size;
        slot * slot_size + RECORD_OFFSET
    }
}

impl Drop for BTreePage {
    fn drop(&mut self) {
        let mut lock = self.tx.lock().unwrap();
        lock.unpin(&self.block);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        index::btree::btree_page::BTreePage,
        metadata::index_manager::{INDEX_BLOCK_SLOT_COLUMN, INDEX_VALUE_COLUMN},
        record::{
            field::{Spec, Value},
            layout::Layout,
            record_page::Slot,
            schema::Schema,
        },
    };

    #[test]
    fn test_btree_page_find_slot_before() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 4096;
        let num_buffers = 256;
        let db = SimpleDB::new(temp_dir, block_size, num_buffers)?;
        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let block = db.file_manager.lock().unwrap().append_block("testfile")?;

        let mut schema = Schema::new();
        schema.add_field(INDEX_VALUE_COLUMN, &Spec::I32);
        schema.add_i32_field(INDEX_BLOCK_SLOT_COLUMN);
        let layout = Layout::new(schema);

        let btree_page = BTreePage::new(tx.clone(), block, layout.clone())?;
        assert_eq!(btree_page.get_num_records()?, 0);
        assert_eq!(btree_page.is_full()?, false);
        for i in 0..50 {
            btree_page.insert_directory(i, Value::I32(i as i32), i)?;
        }

        assert_eq!(Slot::Start, btree_page.find_slot_before(&Value::I32(0))?);
        assert_eq!(Slot::Index(0), btree_page.find_slot_before(&Value::I32(1))?);
        assert_eq!(
            Slot::Index(48),
            btree_page.find_slot_before(&Value::I32(49))?
        );
        assert_eq!(
            Slot::Index(49),
            btree_page.find_slot_before(&Value::I32(50))?
        );
        Ok(())
    }
}
