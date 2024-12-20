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

use super::Scan;

pub struct TableScan {
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
            tx.lock().unwrap().pin(&block)?;
            let mut rp = RecordPage::new(tx, block, layout);
            if is_new {
                rp.format()?;
            }
            rp
        };

        Ok(Self {
            file_name,
            record_page,
            current_slot: Slot::Start,
        })
    }

    pub fn get_block_number(&self) -> usize {
        self.record_page.block.block_slot
    }
}

impl Scan for TableScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        let block = BlockId::get_first_block(&self.file_name);
        self.record_page
            .tx
            .lock()
            .unwrap()
            .unpin(&self.record_page.block);
        self.record_page.tx.lock().unwrap().pin(&block)?;
        self.record_page.reset_block(block);
        self.current_slot = Slot::Start;
        Ok(())
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        loop {
            self.current_slot = self.record_page.next_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(true),
                Slot::Start => unreachable!(),
                Slot::End => {
                    if self
                        .record_page
                        .tx
                        .lock()
                        .unwrap()
                        .is_last_block(&self.record_page.block)?
                    {
                        return Ok(false);
                    } else {
                        let new_block = self.record_page.block.get_next_block();
                        let mut lock = self.record_page.tx.lock().unwrap();
                        lock.unpin(&self.record_page.block);
                        lock.pin(&new_block)?;
                        drop(lock);
                        self.record_page.reset_block(new_block);
                        self.current_slot = Slot::Start;
                    }
                }
            }
        }
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.record_page
            .get_i32(self.current_slot.get_index(), field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.record_page
            .get_string(self.current_slot.get_index(), field_name)
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

    fn close(&mut self) -> Result<(), TransactionError> {
        self.record_page
            .tx
            .lock()
            .unwrap()
            .unpin(&self.record_page.block);
        Ok(())
    }

    fn set_i32(&mut self, field_name: &str, value: i32) -> Result<(), TransactionError> {
        self.record_page
            .set_i32(self.current_slot.get_index(), field_name, value)
    }

    fn set_string(&mut self, field_name: &str, value: &str) -> Result<(), TransactionError> {
        self.record_page
            .set_string(self.current_slot.get_index(), field_name, value)
    }

    fn set_value(&mut self, field_name: &str, value: &Value) -> Result<(), TransactionError> {
        match value {
            Value::I32(i) => self.set_i32(field_name, *i),
            Value::String(s) => self.set_string(field_name, s),
        }
    }

    fn delete(&mut self) -> Result<(), TransactionError> {
        self.record_page.delete(self.current_slot.get_index())
    }

    fn insert(&mut self) -> Result<(), TransactionError> {
        loop {
            self.current_slot = self.record_page.insert_after(self.current_slot)?;
            match self.current_slot {
                Slot::Index(_) => return Ok(()),
                Slot::Start => unreachable!(),
                Slot::End => {
                    let mut lock = self.record_page.tx.lock().unwrap();
                    let next_block = if lock.is_last_block(&self.record_page.block)? {
                        lock.append_block(&self.file_name)?
                    } else {
                        self.record_page.block.get_next_block()
                    };

                    lock.unpin(&self.record_page.block);
                    lock.pin(&next_block)?;
                    drop(lock);
                    self.record_page.reset_block(next_block);
                    self.current_slot = Slot::Start;
                }
            }
        }
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
            let value = table_scan
                .record_page
                .tx
                .lock()
                .unwrap()
                .get_i32(&table_scan.record_page.block, 0)?;
            assert_eq!(value, 1);
        }

        table_scan.before_first()?;
        let value = table_scan
            .record_page
            .tx
            .lock()
            .unwrap()
            .get_i32(&table_scan.record_page.block, 0)?;
        assert_eq!(value, 1);

        let mut num_scanned_records = 0;
        let mut num_deleted_records = 0;
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
        table_scan.close()?;
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
