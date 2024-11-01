use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use crate::buffer::BufferManager;
use crate::file::BlockId;
use crate::log::manager::LogManager;
use crate::log::record::LogRecord;

use super::concurrency::{ConcurrencyManager, LockTable};

pub struct Transaction {
    buffer_manager: Arc<Mutex<BufferManager>>,
    concurrency_manager: ConcurrencyManager,
    log_manager: Arc<Mutex<LogManager>>,
    pub id: usize,
    block_to_buffer_map: HashMap<BlockId, usize>,
    pinned_blocks: Vec<BlockId>,
}

static TRANSACTION_ID: AtomicUsize = AtomicUsize::new(0);

impl Transaction {
    pub fn new(
        log_manager: Arc<Mutex<LogManager>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
        lock_table: Arc<Mutex<LockTable>>,
    ) -> Result<Self, anyhow::Error> {
        let concurrency_manager = ConcurrencyManager::new(lock_table.clone());
        let id = TRANSACTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok(Transaction {
            buffer_manager,
            concurrency_manager,
            log_manager,
            block_to_buffer_map: HashMap::new(),
            pinned_blocks: Vec::new(),
            id,
        })
    }

    pub fn commit(&mut self) -> Result<(), anyhow::Error> {
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

    pub fn rollback(&mut self) -> Result<(), anyhow::Error> {
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
    pub fn recover(&mut self) -> Result<(), anyhow::Error> {
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

    pub fn pin(&mut self, block: &BlockId) -> Result<(), anyhow::Error> {
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
    pub fn get_i32(&mut self, block: &BlockId, offset: usize) -> Result<i32, anyhow::Error> {
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
    ) -> Result<(), anyhow::Error> {
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
        buffer.page.set_i32(offset, value);
        buffer.set_modified(self.id, log_sequence_number);
        Ok(())
    }

    pub fn undo_update(&mut self, log_record: &LogRecord) -> Result<(), anyhow::Error> {
        match log_record {
            LogRecord::SetI32(_, block, offset, old_value, _) => {
                self.pin(block)?;
                self.set_i32(block, *offset, *old_value, false)?;
                self.unpin(block);
            }
            _ => {
                panic!("unexpected log record: {:?}", log_record);
            }
        }
        Ok(())
    }

    fn do_rollback(&mut self) -> Result<(), anyhow::Error> {
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter()?;
        let mut log_records = vec![];

        for log_record in log_iter {
            if log_record.get_transaction_id() == self.id {
                log_records.push(log_record.clone());
                if let LogRecord::Start(_) = log_record {
                    break;
                }
            }
        }
        drop(log_manager);
        for log_record in log_records.iter() {
            self.undo_update(&log_record)?;
        }
        Ok(())
    }

    fn do_recover(&mut self) -> Result<(), anyhow::Error> {
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
    use crate::file::FileManager;

    use super::*;

    #[test]
    fn test_transaction() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let file_manager = Arc::new(Mutex::new(FileManager::new(temp_dir, block_size)));
        let lock_table = Arc::new(Mutex::new(LockTable::new(10)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(
            file_manager.clone(),
            log_manager.clone(),
            3,
        )));
        let block = file_manager.lock().unwrap().append_block("testfile")?;

        // tx1 just sets the initial values of the block, and we don't need to log it.
        let mut tx1 = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx1.pin(&block)?;
        tx1.set_i32(&block, 80, 1, false)?;
        tx1.commit()?;

        let mut tx2 = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx2.pin(&block)?;
        let value = tx2.get_i32(&block, 80)?;
        assert_eq!(value, 1);
        tx2.set_i32(&block, 80, 2, true)?;
        tx2.commit()?;

        let mut tx3 = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx3.pin(&block)?;
        assert_eq!(tx3.get_i32(&block, 80)?, 2);
        tx3.set_i32(&block, 80, 9999, true)?;
        assert_eq!(tx3.get_i32(&block, 80)?, 9999);
        tx3.rollback()?;

        let mut tx4 = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx4.pin(&block)?;
        assert_eq!(tx4.get_i32(&block, 80)?, 2);
        tx4.commit()?;
        Ok(())
    }
}
