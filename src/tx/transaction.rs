use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use crate::buffer::BufferManager;
use crate::errors::TransactionError;
use crate::file::{BlockId, FileManager};
use crate::log::manager::LogManager;
use crate::log::record::LogRecord;

use super::concurrency::{ConcurrencyManager, LockTable};

pub struct Transaction {
    file_manager: Arc<Mutex<FileManager>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
    concurrency_manager: ConcurrencyManager,
    log_manager: Arc<Mutex<LogManager>>,
    pub id: usize,
    block_to_buffer_map: HashMap<BlockId, usize>,
    pinned_blocks: Vec<BlockId>,
    block_size: usize,
}

static TRANSACTION_ID: AtomicUsize = AtomicUsize::new(0);

impl Transaction {
    pub fn new(
        file_manager: Arc<Mutex<FileManager>>,
        log_manager: Arc<Mutex<LogManager>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
        lock_table: Arc<Mutex<LockTable>>,
    ) -> Result<Self, TransactionError> {
        let concurrency_manager = ConcurrencyManager::new(lock_table.clone());
        let id = TRANSACTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let block_size = file_manager.lock().unwrap().block_size;

        Ok(Transaction {
            file_manager,
            buffer_manager,
            concurrency_manager,
            log_manager,
            block_to_buffer_map: HashMap::new(),
            pinned_blocks: Vec::new(),
            id,
            block_size,
        })
    }

    pub fn commit(&mut self) -> Result<(), TransactionError> {
        {
            let buffer_manager = self.buffer_manager.lock().unwrap();
            buffer_manager.flush_all(self.id)?;
        }

        {
            let mut log_manager = self.log_manager.lock().unwrap();
            let log_sequence_number = log_manager.append_record(&LogRecord::Commit(self.id))?;
            log_manager.flush(log_sequence_number)?;
        }

        self.concurrency_manager.release();
        self.unpin_all();
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<(), TransactionError> {
        self.do_rollback()?;

        {
            let buffer_manager = self.buffer_manager.lock().unwrap();
            buffer_manager.flush_all(self.id)?;
        }

        {
            let mut log_manager = self.log_manager.lock().unwrap();
            let log_sequence_number = log_manager.append_record(&LogRecord::Rollback(self.id))?;
            log_manager.flush(log_sequence_number)?;
        }
        // self.recovery_manager.rollback(self.id)?;
        self.concurrency_manager.release();
        self.unpin_all();
        Ok(())
    }

    // "Quiescent Checkpointing" is currently implemented
    // This method is not thread-safe.
    // TODO: understand why we need to flush all buffers twice.
    pub fn recover(&mut self) -> Result<(), TransactionError> {
        {
            let buffer_manager = self.buffer_manager.lock().unwrap();
            buffer_manager.flush_all(self.id)?;
        }

        self.do_recover()?;
        {
            let buffer_manager = self.buffer_manager.lock().unwrap();
            buffer_manager.flush_all(self.id)?;
        }

        let mut log_manager = self.log_manager.lock().unwrap();
        let log_sequence_number = log_manager.append_record(&LogRecord::Checkpoint(self.id))?;
        log_manager.flush(log_sequence_number)?;
        Ok(())
    }

    pub fn pin(&mut self, block: &BlockId) -> Result<(), TransactionError> {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer_index = buffer_manager.pin(block)?;

        self.block_to_buffer_map.insert(block.clone(), buffer_index);
        self.pinned_blocks.push(block.clone());
        Ok(())
    }

    pub fn unpin(&mut self, block: &BlockId) {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        buffer_manager.unpin(buffer_index);

        let first_index = self.pinned_blocks.iter().position(|b| b == block).unwrap();
        self.pinned_blocks.remove(first_index);

        if !self.pinned_blocks.contains(&block) {
            self.block_to_buffer_map.remove(&block);
        }
    }

    // Block with block_id must be pinned before calling this method.
    pub fn get_i32(&mut self, block: &BlockId, offset: usize) -> Result<i32, TransactionError> {
        self.concurrency_manager.lock_shared(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();

        let buffer_lock = buffer_manager.buffers.lock().unwrap();
        Ok(buffer_lock[buffer_index].page.get_i32(offset))
    }

    // Block with block_id must be pinned before calling this method.
    pub fn set_i32(
        &mut self,
        block: &BlockId,
        offset: usize,
        value: i32,
        is_log_needed: bool,
    ) -> Result<usize, TransactionError> {
        self.concurrency_manager.lock_exclusive(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer = &mut buffer_manager.buffers.lock().unwrap()[buffer_index];
        let log_sequence_number = if is_log_needed {
            let block = buffer
                .block
                .clone()
                .expect("buffer must be assigned to a block");
            let old_value = buffer.page.get_i32(offset);
            let record = LogRecord::SetI32(self.id, block, offset, old_value, value);

            let mut log_manager = self.log_manager.lock().unwrap();
            log_manager.append_record(&record)?
        } else {
            0
        };
        let written_length = buffer.page.set_i32(offset, value);
        buffer.set_modified(self.id, log_sequence_number);
        Ok(written_length)
    }

    // Block with block_id must be pinned before calling this method.
    pub fn get_string(
        &mut self,
        block: &BlockId,
        offset: usize,
    ) -> Result<String, TransactionError> {
        self.concurrency_manager.lock_shared(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer_lock = buffer_manager.buffers.lock().unwrap();
        Ok(buffer_lock[buffer_index]
            .page
            .get_string(offset)
            .0
            .to_string())
    }

    // Block with block_id must be pinned before calling this method.
    pub fn set_string(
        &mut self,
        block: &BlockId,
        offset: usize,
        value: &str,
        is_log_needed: bool,
    ) -> Result<usize, TransactionError> {
        self.concurrency_manager.lock_exclusive(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer = &mut buffer_manager.buffers.lock().unwrap()[buffer_index];
        let log_sequence_number = if is_log_needed {
            let block = buffer
                .block
                .clone()
                .expect("buffer must be assigned to a block");
            let (old_value, _) = buffer.page.get_string(offset);
            let record = LogRecord::SetString(
                self.id,
                block,
                offset,
                old_value.to_string(),
                value.to_string(),
            );
            let mut log_manager = self.log_manager.lock().unwrap();
            log_manager.append_record(&record)?
        } else {
            0
        };
        let written_length = buffer.page.set_string(offset, value);
        buffer.set_modified(self.id, log_sequence_number);
        Ok(written_length)
    }

    pub fn append_block(&mut self, file_name: &str) -> Result<BlockId, TransactionError> {
        let dummy = BlockId::create_dummy(file_name);
        self.concurrency_manager.lock_exclusive(&dummy)?;
        let block = self.file_manager.lock().unwrap().append_block(file_name)?;
        Ok(block)
    }

    pub fn get_block_size(&self) -> usize {
        self.block_size
    }

    pub fn is_last_block(&mut self, block: &BlockId) -> Result<bool, TransactionError> {
        let dummy = BlockId::create_dummy(&block.file_name);
        self.concurrency_manager.lock_shared(&dummy)?;
        Ok(self.get_num_blocks(&block.file_name)? == block.block_slot + 1)
    }

    pub fn get_num_blocks(&mut self, file_name: &str) -> Result<usize, TransactionError> {
        let dummy = BlockId::create_dummy(file_name);
        self.concurrency_manager.lock_shared(&dummy)?;
        Ok(self.file_manager.lock().unwrap().get_num_blocks(file_name))
    }

    fn do_rollback(&mut self) -> Result<(), TransactionError> {
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter()?;
        let mut log_records = vec![];

        for log_record in log_iter {
            if log_record.get_transaction_id() == self.id {
                log_records.push(log_record.clone());
            }
            match log_record {
                LogRecord::Start(_) | LogRecord::Checkpoint(_) => {
                    break;
                }
                _ => {}
            }
        }
        drop(log_manager);
        for log_record in log_records.iter() {
            self.undo_update(&log_record)?;
        }
        Ok(())
    }

    fn do_recover(&mut self) -> Result<(), TransactionError> {
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter()?;
        let mut finshed_transactions = HashSet::new();
        let mut log_records = vec![];

        for log_record in log_iter {
            match log_record {
                LogRecord::Commit(transaction_id) | LogRecord::Rollback(transaction_id) => {
                    finshed_transactions.insert(transaction_id);
                }
                LogRecord::Checkpoint(_) => {
                    break;
                }
                _ => {
                    if !finshed_transactions.contains(&log_record.get_transaction_id()) {
                        log_records.push(log_record);
                    }
                }
            }
        }
        drop(log_manager);
        for log_record in log_records.iter() {
            self.undo_update(&log_record)?;
        }
        Ok(())
    }

    fn undo_update(&mut self, log_record: &LogRecord) -> Result<(), TransactionError> {
        match log_record {
            LogRecord::SetI32(_, block, offset, old_value, _) => {
                self.pin(block)?;
                self.set_i32(block, *offset, *old_value, false)?;
                self.unpin(block);
            }
            LogRecord::SetString(_, block, offset, old_value, _) => {
                self.pin(block)?;
                self.set_string(block, *offset, old_value, false)?;
                self.unpin(block);
            }
            _ => {
                panic!("unexpected log record: {:?}", log_record);
            }
        }
        Ok(())
    }

    fn unpin_all(&mut self) {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        for block_index in self.pinned_blocks.iter() {
            let &buffer_index = self.block_to_buffer_map.get(block_index).unwrap();
            buffer_manager.unpin(buffer_index);
        }
        self.pinned_blocks.clear();
        self.block_to_buffer_map.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::{db::SimpleDB, errors::TransactionError};

    #[test]
    fn test_transaction() -> Result<(), TransactionError> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let db = SimpleDB::new(temp_dir, block_size, 3)?;
        let block = db.file_manager.lock().unwrap().append_block("testfile")?;

        // tx1 just sets the initial values of the block, and we don't need to log it.
        let mut tx1 = db.new_transaction()?;
        tx1.pin(&block)?;
        tx1.set_i32(&block, 80, 1, false)?;
        tx1.set_string(&block, 40, "one", false)?;
        tx1.commit()?;

        let mut tx2 = db.new_transaction()?;
        tx2.pin(&block)?;
        assert_eq!(tx2.get_i32(&block, 80)?, 1);
        assert_eq!(tx2.get_string(&block, 40)?, "one");
        tx2.set_i32(&block, 80, 2, true)?;
        tx2.set_string(&block, 40, "two", true)?;
        tx2.commit()?;

        let mut tx3 = db.new_transaction()?;
        tx3.pin(&block)?;
        assert_eq!(tx3.get_i32(&block, 80)?, 2);
        assert_eq!(tx3.get_string(&block, 40)?, "two");
        tx3.set_i32(&block, 80, 9999, true)?;
        tx3.set_string(&block, 40, "dummy", true)?;
        assert_eq!(tx3.get_i32(&block, 80)?, 9999);
        assert_eq!(tx3.get_string(&block, 40)?, "dummy");
        tx3.rollback()?;

        let mut tx4 = db.new_transaction()?;
        tx4.pin(&block)?;
        assert_eq!(tx4.get_i32(&block, 80)?, 2);
        assert_eq!(tx4.get_string(&block, 40)?, "two");
        tx4.commit()?;
        Ok(())
    }
}
