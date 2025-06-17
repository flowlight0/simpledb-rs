use std::sync::{Arc, Mutex};

use crate::{errors::TransactionError, file::BlockId, tx::transaction::Transaction};

use super::layout::Layout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VacancyFlag {
    Empty,
    Full,
}

impl VacancyFlag {
    fn to_bit(&self) -> i32 {
        match self {
            VacancyFlag::Empty => 0,
            VacancyFlag::Full => 1,
        }
    }
}

// RecordPage is a struct that represents a page in a record file. It is used to read and write records to the page.
// It contains a reference to a transaction, a block id, and a layout.
// The layout is used to determine the size and offset of each field in the record.
// RecordPage pins the block in the transaction when it is created and unpins the block when it is dropped.
pub struct RecordPage {
    tx: Arc<Mutex<Transaction>>,
    pub block: BlockId,
    pub layout: Arc<Layout>,
    num_slots: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Index(usize),
    Start,
    End,
}

impl Slot {
    pub fn index(&self) -> usize {
        match self {
            Slot::Index(index) => *index,
            Slot::Start => panic!("Slot::index() called on Slot::Start"),
            Slot::End => panic!("Slot::index() called on Slot::End"),
        }
    }

    pub fn next(&self) -> Slot {
        match self {
            Slot::Index(index) => Slot::Index(index + 1),
            Slot::Start => Slot::Index(0),
            Slot::End => Slot::End,
        }
    }

    pub fn prev(&self) -> Slot {
        match self {
            Slot::Index(0) => Slot::Start,
            Slot::Index(index) => Slot::Index(index - 1),
            Slot::Start => Slot::Start,
            Slot::End => panic!("Slot::prev() called on Slot::End"),
        }
    }
}

impl RecordPage {
    pub fn new(tx: Arc<Mutex<Transaction>>, block: BlockId, layout: Arc<Layout>) -> Self {
        tx.lock().unwrap().pin(&block).unwrap();
        let block_size = tx.lock().unwrap().get_block_size();
        let num_slots = block_size / layout.slot_size;
        RecordPage {
            tx,
            block,
            layout,
            num_slots,
        }
    }

    pub fn reset_block(&mut self, block: BlockId) -> Result<(), TransactionError> {
        self.tx.lock().unwrap().unpin(&self.block);
        self.tx.lock().unwrap().pin(&block)?;
        self.block = block;
        let block_size = self.tx.lock().unwrap().get_block_size();
        self.num_slots = block_size / self.layout.slot_size;
        Ok(())
    }

    pub fn get_next_block(
        &mut self,
        append_if_neccessary: bool,
    ) -> Result<Option<BlockId>, TransactionError> {
        let mut lock = self.tx.lock().unwrap();
        if lock.is_last_block(&self.block)? {
            if append_if_neccessary {
                let new_block = lock.append_block(&self.block.file_name)?;
                Ok(Some(new_block))
            } else {
                Ok(None)
            }
        } else {
            let new_block = self.block.get_next_block();
            Ok(Some(new_block))
        }
    }

    pub fn get_i32(&mut self, slot: usize, field_name: &str) -> Result<i32, TransactionError> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .lock()
            .unwrap()
            .get_i32(&self.block, self.offset(slot) + field_offset)
    }

    pub fn set_i32(
        &mut self,
        slot: usize,
        field_name: &str,
        value: i32,
    ) -> Result<(), TransactionError> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx.lock().unwrap().set_i32(
            &self.block,
            self.offset(slot) + field_offset,
            value,
            true,
        )?;
        self.set_not_null(slot, field_name)?;
        Ok(())
    }

    pub fn get_string(
        &mut self,
        slot: usize,
        field_name: &str,
    ) -> Result<String, TransactionError> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .lock()
            .unwrap()
            .get_string(&self.block, self.offset(slot) + field_offset)
    }

    pub fn set_string(
        &mut self,
        slot: usize,
        field_name: &str,
        value: &str,
    ) -> Result<(), TransactionError> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx.lock().unwrap().set_string(
            &self.block,
            self.offset(slot) + field_offset,
            value,
            true,
        )?;
        self.set_not_null(slot, field_name)?;
        Ok(())
    }

    pub fn set_null(&mut self, slot: usize, field_name: &str) -> Result<(), TransactionError> {
        let bit = self.layout.null_bit_location(field_name);
        let mut flags = self
            .tx
            .lock()
            .unwrap()
            .get_i32(&self.block, self.offset(slot))?;
        flags |= 1 << bit;
        self.tx
            .lock()
            .unwrap()
            .set_i32(&self.block, self.offset(slot), flags, true)?;
        Ok(())
    }

    pub fn is_null(&mut self, slot: usize, field_name: &str) -> Result<bool, TransactionError> {
        let bit = self.layout.null_bit_location(field_name);
        let flags = self
            .tx
            .lock()
            .unwrap()
            .get_i32(&self.block, self.offset(slot))?;
        Ok((flags & (1 << bit)) != 0)
    }

    pub fn delete(&mut self, slot: usize) -> Result<(), TransactionError> {
        self.set_vacancy_flag(slot, VacancyFlag::Empty)
    }

    pub fn insert_after(&mut self, slot: Slot) -> Result<Slot, TransactionError> {
        let result = self.search_after(slot, VacancyFlag::Empty);
        if let Ok(Slot::Index(index)) = result {
            self.set_vacancy_flag(index, VacancyFlag::Full)?;
            Ok(Slot::Index(index))
        } else {
            result
        }
    }

    pub fn next_after(&mut self, slot: Slot) -> Result<Slot, TransactionError> {
        self.search_after(slot, VacancyFlag::Full)
    }

    pub fn prev_before(&mut self, slot: Slot) -> Result<Slot, TransactionError> {
        self.search_before(slot, VacancyFlag::Full)
    }

    pub fn after_last(&self) -> Slot {
        Slot::End
    }

    fn search_after(
        &mut self,
        slot: Slot,
        vacancy_flag: VacancyFlag,
    ) -> Result<Slot, TransactionError> {
        if slot == Slot::End {
            return Ok(Slot::End);
        }

        let mut next_index = match slot {
            Slot::Index(index) => index + 1,
            Slot::Start => 0,
            Slot::End => unreachable!(),
        };

        while self.is_valid_slot(next_index) {
            let slot_flags = self
                .tx
                .lock()
                .unwrap()
                .get_i32(&self.block, self.offset(next_index))?;
            if (slot_flags & 1) == vacancy_flag.to_bit() {
                return Ok(Slot::Index(next_index));
            }
            next_index += 1;
        }
        Ok(Slot::End)
    }

    fn search_before(
        &mut self,
        slot: Slot,
        vacancy_flag: VacancyFlag,
    ) -> Result<Slot, TransactionError> {
        if slot == Slot::Start {
            return Ok(Slot::Start);
        }

        let mut prev_index = match slot {
            Slot::Index(index) => {
                if index == 0 {
                    return Ok(Slot::Start);
                }
                index - 1
            }
            Slot::Start => unreachable!(),
            Slot::End => {
                if self.num_slots == 0 {
                    return Ok(Slot::Start);
                }
                self.num_slots - 1
            }
        };

        loop {
            let slot_flags = self
                .tx
                .lock()
                .unwrap()
                .get_i32(&self.block, self.offset(prev_index))?;
            if (slot_flags & 1) == vacancy_flag.to_bit() {
                return Ok(Slot::Index(prev_index));
            }
            if prev_index == 0 {
                break;
            }
            prev_index -= 1;
        }
        Ok(Slot::Start)
    }

    pub fn format(&mut self) -> Result<(), TransactionError> {
        let mut slot = 0;

        while self.is_valid_slot(slot) {
            // while self.offset(slot + 1) <= self.tx.get_block_size() {
            self.tx.lock().unwrap().set_i32(
                &self.block,
                self.offset(slot),
                VacancyFlag::Empty.to_bit(),
                false,
            )?;

            let schema = self.layout.schema.clone();
            for field_name in &schema.i32_fields {
                self.tx.lock().unwrap().set_i32(
                    &self.block,
                    self.offset(slot) + self.layout.get_offset(field_name),
                    0,
                    false,
                )?;
                self.set_not_null(slot, field_name)?;
            }

            for field_name in &schema.string_fields {
                self.tx.lock().unwrap().set_string(
                    &self.block,
                    self.offset(slot) + self.layout.get_offset(field_name),
                    "",
                    false,
                )?;
                self.set_not_null(slot, field_name)?;
            }
            slot += 1;
        }
        Ok(())
    }

    fn is_valid_slot(&self, slot: usize) -> bool {
        slot < self.num_slots
    }

    fn offset(&self, slot: usize) -> usize {
        slot * self.layout.slot_size
    }

    fn set_vacancy_flag(
        &mut self,
        slot: usize,
        vacancy_flag: VacancyFlag,
    ) -> Result<(), TransactionError> {
        let mut flags = self
            .tx
            .lock()
            .unwrap()
            .get_i32(&self.block, self.offset(slot))?;

        match vacancy_flag {
            VacancyFlag::Empty => flags &= !1, // Clear the vacancy flag
            VacancyFlag::Full => flags |= 1,   // Set the vacancy flag
        }

        self.tx
            .lock()
            .unwrap()
            .set_i32(&self.block, self.offset(slot), flags, true)?;
        Ok(())
    }

    fn set_not_null(&mut self, slot: usize, field_name: &str) -> Result<(), TransactionError> {
        let bit = self.layout.null_bit_location(field_name);
        let mut flags = self
            .tx
            .lock()
            .unwrap()
            .get_i32(&self.block, self.offset(slot))?;
        flags &= !(1 << bit);
        self.tx
            .lock()
            .unwrap()
            .set_i32(&self.block, self.offset(slot), flags, true)?;
        Ok(())
    }
}

impl Drop for RecordPage {
    fn drop(&mut self) {
        self.tx.lock().unwrap().unpin(&self.block);
    }
}

#[cfg(test)]
mod tests {

    use std::sync::{Arc, Mutex};

    use crate::{
        db::SimpleDB,
        errors::TransactionError,
        record::{layout::Layout, schema::Schema},
    };

    use super::{RecordPage, Slot};

    #[test]
    fn test_record_page() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 4096;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let block = db.file_manager.lock().unwrap().append_block("testfile")?;
        let mut record_page = RecordPage::new(tx.clone(), block.clone(), layout.clone());
        record_page.format()?;

        let mut slot = Slot::Start;
        for i in 0..10 {
            slot = record_page.insert_after(slot)?;
            record_page.set_i32(slot.index(), "A", i)?;
            record_page.set_string(slot.index(), "B", &i.to_string())?;
        }

        slot = Slot::Start;
        for _ in 0..10 {
            slot = record_page.next_after(slot)?;
            let value = record_page.get_i32(slot.index(), "A")?;
            if value % 2 == 0 {
                record_page.delete(slot.index())?;
            }
        }

        slot = Slot::Start;
        for i in 0..5 {
            slot = record_page.next_after(slot)?;
            let i32_value = record_page.get_i32(slot.index(), "A")?;
            assert_eq!(i32_value, i * 2 + 1);
            let string_value = record_page.get_string(slot.index(), "B")?;
            assert_eq!(string_value, (i * 2 + 1).to_string());
        }
        assert_eq!(record_page.next_after(slot)?, Slot::End);
        drop(record_page);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_record_page_previous() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let block = db.file_manager.lock().unwrap().append_block("testfile2")?;
        let mut record_page = RecordPage::new(tx.clone(), block.clone(), layout.clone());
        record_page.format()?;

        let mut slot = Slot::Start;
        for i in 0..5 {
            slot = record_page.insert_after(slot)?;
            record_page.set_i32(slot.index(), "A", i)?;
        }

        slot = record_page.after_last();
        for expected in (0..5).rev() {
            slot = record_page.prev_before(slot)?;
            let v = record_page.get_i32(slot.index(), "A")?;
            assert_eq!(v, expected);
        }
        assert_eq!(record_page.prev_before(slot)?, Slot::Start);
        drop(record_page);
        tx.lock().unwrap().commit()?;
        Ok(())
    }

    #[test]
    fn test_record_page_null_handling() -> Result<(), TransactionError> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        let layout = Arc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 1024;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let block = db.file_manager.lock().unwrap().append_block("testfile3")?;
        let mut record_page = RecordPage::new(tx.clone(), block.clone(), layout.clone());
        record_page.format()?;

        let slot = record_page.insert_after(Slot::Start)?;
        assert!(!record_page.is_null(slot.index(), "A")?);
        record_page.set_null(slot.index(), "A")?;
        assert!(record_page.is_null(slot.index(), "A")?);
        record_page.set_i32(slot.index(), "A", 10)?;
        assert!(!record_page.is_null(slot.index(), "A")?);
        drop(record_page);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
