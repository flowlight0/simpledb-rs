use std::sync::{Arc, Mutex};

use crate::{
    errors::TransactionError,
    file::BlockId,
    record::{
        field::{Type, Value},
        layout::Layout,
        record_page::{RecordPage, Slot},
    },
    scan::ScanControl,
    tx::transaction::Transaction,
};

pub struct ChunkScan {
    buffers: Vec<RecordPage>,
    layout: Arc<Layout>,
    block_slot_start: usize,
    block_slot_end: usize,
    current_record_page_index: usize,
    current_record_slot: Slot,
}

impl ChunkScan {
    pub fn new(
        tx: Arc<Mutex<Transaction>>,
        layout: Arc<Layout>,
        block_file_name: &str,
        block_slot_start: usize,
        block_slot_end: usize,
    ) -> Result<Self, TransactionError> {
        let mut buffers = vec![];
        for block_slot in block_slot_start..block_slot_end {
            let block = BlockId::new(block_file_name, block_slot);
            let record_page = RecordPage::new(tx.clone(), block, layout.clone());
            buffers.push(record_page);
        }

        let mut scan = ChunkScan {
            buffers,
            layout,
            block_slot_start,
            block_slot_end,
            current_record_page_index: 0,
            current_record_slot: Slot::Start,
        };
        scan.before_first()?;
        Ok(scan)
    }

    fn move_to_block_slot(&mut self, block_slot: usize) -> Result<(), TransactionError> {
        let record_page_index = block_slot - self.block_slot_start;
        self.current_record_page_index = record_page_index;
        self.current_record_slot = Slot::Start;
        Ok(())
    }
}

impl ScanControl for ChunkScan {
    fn before_first(&mut self) -> Result<(), TransactionError> {
        self.move_to_block_slot(self.block_slot_start)
    }

    fn next(&mut self) -> Result<bool, TransactionError> {
        loop {
            self.current_record_slot = self.buffers[self.current_record_page_index]
                .next_after(self.current_record_slot)?;
            match self.current_record_slot {
                Slot::Index(_) => return Ok(true),
                Slot::End => {
                    let current_block_slot = self.current_record_page_index + self.block_slot_start;
                    if current_block_slot + 1 < self.block_slot_end {
                        self.move_to_block_slot(current_block_slot + 1)?;
                    } else {
                        return Ok(false);
                    }
                }
                Slot::Start => unreachable!(),
            }
        }
    }

    fn get_i32(&mut self, field_name: &str) -> Result<i32, TransactionError> {
        self.buffers[self.current_record_page_index]
            .get_i32(self.current_record_slot.index(), field_name)
    }

    fn get_string(&mut self, field_name: &str) -> Result<String, TransactionError> {
        self.buffers[self.current_record_page_index]
            .get_string(self.current_record_slot.index(), field_name)
    }

    fn get_value(&mut self, field_name: &str) -> Result<Value, TransactionError> {
        match self.layout.schema.get_field_type(field_name) {
            Type::I32 => Ok(Value::I32(self.get_i32(field_name)?)),
            Type::String => Ok(Value::String(self.get_string(field_name)?)),
        }
    }

    fn has_field(&self, field_name: &str) -> bool {
        self.layout.has_field(field_name)
    }
}
