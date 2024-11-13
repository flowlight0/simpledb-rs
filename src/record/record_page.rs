use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{file::BlockId, tx::transaction::Transaction};

use super::layout::Layout;

const EMPTY: i32 = 0;
const FULL: i32 = 1;

pub struct RecordPage {
    pub tx: Arc<Mutex<Transaction>>,
    pub block: BlockId,
    pub layout: Rc<Layout>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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

    pub fn get_index(&self) -> usize {
        match self {
            Slot::Index(index) => *index,
            _ => panic!("Slot::get_index() called on Slot::Start or Slot::End"),
        }
    }
}

impl RecordPage {
    pub fn new(tx: Arc<Mutex<Transaction>>, block: BlockId, layout: Rc<Layout>) -> Self {
        RecordPage { tx, block, layout }
    }

    pub fn reset_block(&mut self, block: BlockId) {
        self.block = block;
    }

    pub fn get_i32(&mut self, slot: usize, field_name: &str) -> Result<i32, anyhow::Error> {
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
    ) -> Result<(), anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx.lock().unwrap().set_i32(
            &self.block,
            self.offset(slot) + field_offset,
            value,
            true,
        )?;
        Ok(())
    }

    pub fn get_string(&mut self, slot: usize, field_name: &str) -> Result<String, anyhow::Error> {
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
    ) -> Result<(), anyhow::Error> {
        let field_offset = self.layout.get_offset(field_name);
        self.tx.lock().unwrap().set_string(
            &self.block,
            self.offset(slot) + field_offset,
            value,
            true,
        )?;
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
            let slot_flag = self
                .tx
                .lock()
                .unwrap()
                .get_i32(&self.block, self.offset(next_index))?;
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
                .lock()
                .unwrap()
                .set_i32(&self.block, self.offset(slot), EMPTY, false)?;

            let schema = self.layout.schema.clone();
            for field_name in &schema.i32_fields {
                self.tx.lock().unwrap().set_i32(
                    &self.block,
                    self.offset(slot) + self.layout.get_offset(field_name),
                    0,
                    false,
                )?;
            }

            for field_name in &schema.string_fields {
                self.tx.lock().unwrap().set_string(
                    &self.block,
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
        self.offset(slot + 1) <= self.tx.lock().unwrap().get_block_size()
    }

    fn offset(&self, slot: usize) -> usize {
        slot * self.layout.slot_size
    }

    fn set_flag(&mut self, slot: usize, value: i32) -> Result<(), anyhow::Error> {
        self.tx
            .lock()
            .unwrap()
            .set_i32(&self.block, self.offset(slot), value, true)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::{
        rc::Rc,
        sync::{Arc, Mutex},
    };

    use crate::{
        db::SimpleDB,
        record::{layout::Layout, schema::Schema},
    };

    use super::{RecordPage, Slot};

    #[test]
    fn test_record_page() -> Result<(), anyhow::Error> {
        let mut schema = Schema::new();
        schema.add_i32_field("A");
        schema.add_string_field("B", 20);
        let layout = Rc::new(Layout::new(schema));

        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 4096;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;

        let tx = Arc::new(Mutex::new(db.new_transaction()?));
        let block = db.file_manager.lock().unwrap().append_block("testfile")?;
        tx.lock().unwrap().pin(&block)?;
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

        tx.lock().unwrap().unpin(&block);
        tx.lock().unwrap().commit()?;
        Ok(())
    }
}
