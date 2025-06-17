use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    record::{
        field::{Type, Value},
        layout::Layout,
        record_page::{RecordPage, Slot},
    },
    tx::transaction::Transaction,
};

use super::{RecordPointer, ScanControl};

pub struct TableScan {
    tx: Arc<Mutex<Transaction>>, 
    file_name: String,
    record_page: RecordPage,
    current_slot: Slot,
}

impl TableScan {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        table_name: &str,
        layout: Arc<Layout>,
    ) -> Result<Self, TransactionError> {
        let file_name = format!("{}.tbl", table_name);

        let record_page = {
            let num_blocks = tx.lock().unwrap().get_num_blocks(&file_name)?;
            let (block, is_new) = if num_blocks == 0 {
                (tx.lock().unwrap().append_block(&file_name)?, true)
            } else {
                (BlockId::get_first_block(&file_name), false)
            };
            let mut rp = RecordPage::new(tx.clone(), block, layout);
            if is_new {
                rp.format()?;
            }
            rp
        };

        let block_size = tx.lock().unwrap().get_block_size();
        let slot_size = record_page.layout.slot_size;
        if slot_size > block_size {
            return Err(TransactionError::TooSmallBlockError(block_size, slot_size));
        }

        Ok(Self {
            tx,
            file_name,
            record_page,
            current_slot: Slot::Start,
        })
    }

    pub fn get_block_slot(&self) -> usize {
        self.record_page.block.block_slot
    }

    pub fn after_last(&mut self) -> Result<(), TransactionError> {
        let num_blocks = self.tx.lock().unwrap().get_num_blocks(&self.file_name)?;
        let block = BlockId::new(&self.file_name, num_blocks - 1);
        self.record_page.reset_block(block)?;
        self.current_slot = self.record_page.after_last();
        Ok(())
    }

    pub fn previous(&mut self) -> Result<bool, TransactionError> {
        loop {
            self.current_slot = self.record_page.prev_before(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(true),
                Slot::Start => {
                    if let Some(prev_block) = self.record_page.block.get_previous_block() {
                        self.record_page.reset_block(prev_block)?;
                        self.current_slot = self.record_page.after_last();
                    } else {
                        return Ok(false);
                    }
                }
                Slot::End => unreachable!(),
            }
        }
    }
}

impl ScanControl for TableScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        let block = BlockId::get_first_block(&self.file_name);
        self.record_page.reset_block(block)?;
        self.current_slot = Slot::Start;
        Ok(())
    }

    fn after_last(&mut self) -> Result<(), TransactionError> {
        TableScan::after_last(self)
    }

    fn previous(&mut self) -> Result<bool, TransactionError> {
        TableScan::previous(self)
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        loop {
            self.current_slot = self.record_page.next_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(true),
                Slot::Start => unreachable!(),
                Slot::End => {
                    if let Some(new_block) = self.record_page.get_next_block(false)? {
                        self.record_page.reset_block(new_block)?;
                        self.current_slot = Slot::Start;
                    } else {
                        return Ok(false);
                    }
                }
            }
        }
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.record_page
            .get_i32(self.current_slot.index(), field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.record_page
            .get_string(self.current_slot.index(), field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        match self.record_page.layout.schema.get_field_type(field_name) {
            Type::I32 => Ok(Value::I32(self.get_i32(field_name)?)),
            Type::String => Ok(Value::String(self.get_string(field_name)?)),
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.record_page.layout.has_field(field_name)
    }

    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), TransactionError> {
        self.record_page
            .set_i32(self.current_slot.index(), field_name, value)
    }

    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), TransactionError> {
        self.record_page
            .set_string(self.current_slot.index(), field_name, value)
    }

    fn set_value(&mut self, field_name: &str, value: &Value) -> Result<(), TransactionError> {
        match value {
            Value::I32(i) => self.set_i32(field_name, *i),
            Value::String(s) => self.set_string(field_name, s),
        }
    }

    fn delete(&mut self) -> Result<(), TransactionError> {
        self.record_page.delete(self.current_slot.index())
    }

    fn insert(&mut self) -> Result<(), TransactionError> {
        loop {
            self.current_slot = self.record_page.insert_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(()),
                Slot::Start => unreachable!(),
                Slot::End => {
                    let next_block = self
                        .record_page
                        .get_next_block(true)?
                        .expect("next block must have been appended before reaching here");
                    self.record_page.reset_block(next_block)?;
                    self.current_slot = Slot::Start;
                }
            }
        }
    }

    fn get_record_pointer(&self) -> RecordPointer {
        RecordPointer(self.get_block_slot(), self.current_slot)
    }

    fn move_to_record_pointer(
        &mut self,
        record_id: &RecordPointer,
    ) -> Result<(), TransactionError> {
        let new_block = BlockId::new(&self.file_name, record_id.0);
        self.record_page.reset_block(new_block)?;
        self.current_slot = record_id.1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::{db::SimpleDB, record::schema::Schema};

    use super::*;

    #[test]
    fn test_table_scan() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan = TableScan::new(tx.clone(), "testtable", layout.clone())?;
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
            table_scan.set_string("B", &i.to_string())?;

            assert_eq!(table_scan.get_i32("A")?, i);
            assert_eq!(table_scan.get_string("B")?, i.to_string());
        }

        let mut num_scanned_records = 0;
        let mut num_deleted_records = 0;
        table_scan.before_first()?;
        while table_scan.next()? {
            let value = table_scan.get_i32("A")?;
            num_scanned_records += 1;
            if value % 2 == 0 {
                table_scan.delete()?;
                num_deleted_records += 1;
            }
        }
        assert_eq!(num_scanned_records, 50);
        assert_eq!(num_deleted_records, 25);

        table_scan.before_first()?;
        for i in 0..25 {
            assert!(table_scan.next()?);
            let i32_value = table_scan.get_i32("A")?;
            assert_eq!(i32_value, i * 2 + 1);
            let string_value = table_scan.get_string("B")?;
            assert_eq!(string_value, (i * 2 + 1).to_string());
        }
        assert!(!table_scan.next()?);
        drop(table_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_table_scan_previous() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let mut table_scan = TableScan::new(tx.clone(), "testtable2", layout.clone())?;
        table_scan.before_first()?;
        for i in 0..10 {
            table_scan.insert()?;
            table_scan.set_i32("A", i)?;
        }

        table_scan.after_last()?;
        for expected in (0..10).rev() {
            assert!(table_scan.previous()?);
            assert_eq!(table_scan.get_i32("A")?, expected);
        }
        assert!(!table_scan.previous()?);
        drop(table_scan);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
