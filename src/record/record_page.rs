use crate::{file::BlockId, tx::transaction::Transaction};

use super::layout::Layout;

const EMPTY: i32 = 0;
const FULL: i32 = 1;

pub struct RecordPage<'a> {
    tx: &'a mut Transaction,
    block: &'a BlockId,
    layout: &'a Layout<'a>,
}

#[derive(Debug, PartialEq)]
pub enum Slot {
    Index(usize),
    Start,
    End,
}

impl Slot {
    pub fn index(&self) -> usize {
        match self {
            Slot::Index(index) => *index,
            _ => panic!("Slot::index() called on Slot::Start or Slot::End"),
        }
    }

    pub fn next(&self) -> Slot {
        match self {
            Slot::Index(index) => Slot::Index(index + 1),
            Slot::Start => Slot::Index(0),
            Slot::End => Slot::End,
        }
    }
}

impl<'a> RecordPage<'a> {
    pub fn new(tx: &'a mut Transaction, block: &'a BlockId, layout: &'a Layout) -> Self {
        RecordPage { tx, block, layout }
    }

    pub fn get_i32(&mut self, slot: usize, field_name: &str) -> Result<i32, anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .get_i32(self.block, self.offset(slot) + field_offset)
    }

    pub fn set_i32(
        &mut self,
        slot: usize,
        field_name: &str,
        value: i32,
    ) -> Result<(), anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .set_i32(self.block, self.offset(slot) + field_offset, value, true)?;
        Ok(())
    }

    pub fn get_string(&mut self, slot: usize, field_name: &str) -> Result<String, anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .get_string(self.block, self.offset(slot) + field_offset)
    }

    pub fn set_string(
        &mut self,
        slot: usize,
        field_name: &str,
        value: &str,
    ) -> Result<(), anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx
            .set_string(self.block, self.offset(slot) + field_offset, value, true)?;
        Ok(())
    }

    pub fn delete(&mut self, slot: usize) -> Result<(), anyhow::Error> {
        self.set_flag(slot, EMPTY)
    }

    pub fn insert_after(&mut self, slot: Slot) -> Result<Slot, anyhow::Error> {
        let result = self.search_after(slot, EMPTY);
        if let Ok(Slot::Index(index)) = result {
            self.set_flag(index, FULL)?;
            Ok(Slot::Index(index))
        } else {
            result
        }
    }

    pub fn next_after(&mut self, slot: Slot) -> Result<Slot, anyhow::Error> {
        self.search_after(slot, FULL)
    }

    fn search_after(&mut self, slot: Slot, flag: i32) -> Result<Slot, anyhow::Error> {
        if slot == Slot::End {
            return Ok(Slot::End);
        }

        let mut next_index = match slot {
            Slot::Index(index) => index + 1,
            Slot::Start => 0,
            Slot::End => unreachable!(),
        };

        while self.is_valid_slot(next_index) {
            let slot_flag = self.tx.get_i32(self.block, self.offset(next_index))?;
            if slot_flag == flag {
                return Ok(Slot::Index(next_index));
            }
            next_index += 1;
        }
        Ok(Slot::End)
    }

    pub fn format(&mut self) -> Result<(), anyhow::Error> {
        let mut slot = 0;
        while self.is_valid_slot(slot) {
            // while self.offset(slot + 1) <= self.tx.get_block_size() {
            self.tx
                .set_i32(self.block, self.offset(slot), EMPTY, false)?;

            let schema = self.layout.schema;
            for field_name in &schema.i32_fields {
                self.tx.set_i32(
                    self.block,
                    self.offset(slot) + self.layout.get_offset(field_name),
                    0,
                    false,
                )?;
            }

            for field_name in &schema.string_fields {
                self.tx.set_string(
                    self.block,
                    self.offset(slot) + self.layout.get_offset(field_name),
                    "",
                    false,
                )?;
            }
            slot += 1;
        }
        Ok(())
    }

    fn is_valid_slot(&self, slot: usize) -> bool {
        self.offset(slot + 1) <= self.tx.get_block_size()
    }

    fn offset(&self, slot: usize) -> usize {
        slot * self.layout.slot_size
    }

    fn set_flag(&mut self, slot: usize, value: i32) -> Result<(), anyhow::Error> {
        self.tx
            .set_i32(self.block, self.offset(slot), value, true)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::{
        buffer::BufferManager,
        file::FileManager,
        log::manager::LogManager,
        record::{layout::Layout, schema::Schema},
        tx::{concurrency::LockTable, transaction::Transaction},
    };

    use super::{RecordPage, Slot};

    #[test]
    fn test_record_page() -> Result<(), anyhow::Error> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Layout::new(&schema);

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 4096;
        let file_manager = Arc::new(Mutex::new(FileManager::new(temp_dir, block_size)));
        let lock_table = Arc::new(Mutex::new(LockTable::new(10)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(
            file_manager.clone(),
            log_manager.clone(),
            3,
        )));

        let mut tx = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;

        let block = file_manager.lock().unwrap().append_block("testfile")?;
        tx.pin(&block)?;
        let mut record_page = RecordPage::new(&mut tx, &block, &layout);
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

        tx.unpin(&block);
        tx.commit()?;
        Ok(())
    }
}
