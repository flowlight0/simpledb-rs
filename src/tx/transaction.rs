use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

use anyhow::Ok;

use crate::file::BlockId;
use crate::{
    buffer::BufferManager,
    file::FileManager,
    log::{LogManager, LogRecord},
};

use super::concurrency::{ConcurrencyManager, LockTable};
use super::recovery::RecoveryManager;

pub struct Transaction {
    file_manager: Arc<Mutex<FileManager>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
    concurrency_manager: ConcurrencyManager,
    log_manager: Arc<Mutex<LogManager>>,
    recovery_manager: RecoveryManager,
    pub id: usize,
    block_to_buffer_map: HashMap<BlockId, usize>,
    pinned_blocks: Vec<BlockId>,
}

static TRANSACTION_ID: AtomicUsize = AtomicUsize::new(0);

impl Transaction {
    pub fn new(
        file_manager: Arc<Mutex<FileManager>>,
        log_manager: Arc<Mutex<LogManager>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
        lock_table: Arc<Mutex<LockTable>>,
    ) -> Result<Self, anyhow::Error> {
        let concurrency_manager = ConcurrencyManager::new(lock_table.clone());
        let id = TRANSACTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let recovery_manager = RecoveryManager::new(
            buffer_manager.clone(),
            log_manager.clone(),
            id,
            RefCell::new(Weak::new()),
        )?;

        Ok(Transaction {
            file_manager,
            buffer_manager,
            concurrency_manager,
            log_manager,
            recovery_manager,
            block_to_buffer_map: HashMap::new(),
            pinned_blocks: Vec::new(),
            id,
        })
    }

    pub fn commit(&mut self) -> Result<(), anyhow::Error> {
        self.recovery_manager.commit(self.id)?;
        self.concurrency_manager.release();
        self.unpin_all();
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<(), anyhow::Error> {
        self.recovery_manager.rollback(self.id)?;
        self.concurrency_manager.release();
        self.unpin_all();
        Ok(())
    }

    pub fn recover(&mut self) -> Result<(), anyhow::Error> {
        let buffer_manager = self.buffer_manager.lock().unwrap();
        buffer_manager.flush_all(self.id)?;
        self.recovery_manager.recover()?;
        Ok(())
    }

    pub fn pin(&mut self, block: BlockId) -> Result<(), anyhow::Error> {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer_index = buffer_manager.pin(block)?;

        self.block_to_buffer_map.insert(block, buffer_index);
        self.pinned_blocks.push(block);
        Ok(())
    }

    pub fn unpin(&mut self, block: BlockId) {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        buffer_manager.unpin(buffer_index);

        let first_index = self.pinned_blocks.iter().position(|&b| b == block).unwrap();
        self.pinned_blocks.remove(first_index);

        if !self.pinned_blocks.contains(&block) {
            self.block_to_buffer_map.remove(&block);
        }
    }

    // Block with block_id must be pinned before calling this method.
    pub fn get_i32(&mut self, block: BlockId, offset: usize) -> Result<i32, anyhow::Error> {
        self.concurrency_manager.lock_shared(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();

        let buffer_lock = buffer_manager.buffers.lock().unwrap();
        Ok(buffer_lock[buffer_index].page.get_i32(offset))
    }

    // Block with block_id must be pinned before calling this method.
    pub fn set_i32(
        &mut self,
        block: BlockId,
        offset: usize,
        value: i32,
        is_log_needed: bool,
    ) -> Result<(), anyhow::Error> {
        self.concurrency_manager.lock_exclusive(block)?;
        let &buffer_index = self.block_to_buffer_map.get(&block).unwrap();
        let buffer_manager = self.buffer_manager.lock().unwrap();
        let buffer = &mut buffer_manager.buffers.lock().unwrap()[buffer_index];
        if is_log_needed {
            self.recovery_manager.set_i32(buffer, offset, value)?;
        }
        buffer.page.set_i32(offset, value);
        buffer.set_modified(self.id, 0);
        Ok(())
    }

    pub fn undo(&self, log_record: &LogRecord) -> Result<(), std::io::Error> {
        todo!();
    }

    fn unpin_all(&mut self) {
        let mut buffer_manager = self.buffer_manager.lock().unwrap();
        for &block_index in self.pinned_blocks.iter() {
            let &buffer_index = self.block_to_buffer_map.get(&block_index).unwrap();
            buffer_manager.unpin(buffer_index);
        }
        self.pinned_blocks.clear();
        self.block_to_buffer_map.clear();
    }
}

#[cfg(test)]
mod tests {
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
            file_manager.clone(),
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx1.pin(block)?;
        tx1.set_i32(block, 80, 1, false)?;
        tx1.commit()?;

        let mut tx2 = Transaction::new(
            file_manager.clone(),
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx2.pin(block)?;
        let value = tx2.get_i32(block, 80)?;
        assert_eq!(value, 1);
        tx2.set_i32(block, 80, 2, false)?;
        tx2.commit()?;

        let mut tx3 = Transaction::new(
            file_manager.clone(),
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx3.pin(block)?;
        assert_eq!(tx3.get_i32(block, 80)?, 2);
        tx3.set_i32(block, 80, 9999, true)?;
        assert_eq!(tx3.get_i32(block, 80)?, 9999);
        tx3.rollback()?;

        let mut tx4 = Transaction::new(
            file_manager.clone(),
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;
        tx4.pin(block)?;
        assert_eq!(tx4.get_i32(block, 80)?, 2);
        tx4.commit()?;
        Ok(())
    }
}
