use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{
    buffer::BufferManager,
    file::FileManager,
    log::manager::LogManager,
    tx::{concurrency::LockTable, transaction::Transaction},
};

pub struct SimpleDB {
    pub file_manager: Arc<Mutex<FileManager>>,
    lock_table: Arc<Mutex<LockTable>>,
    log_manager: Arc<Mutex<LogManager>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
}

impl SimpleDB {
    pub fn new(
        directory: PathBuf,
        block_size: usize,
        num_buffers: usize,
    ) -> Result<SimpleDB, anyhow::Error> {
        let file_manager = Arc::new(Mutex::new(FileManager::new(directory, block_size)));
        let lock_table = Arc::new(Mutex::new(LockTable::new(10)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(
            file_manager.clone(),
            log_manager.clone(),
            num_buffers,
        )));
        Ok(SimpleDB {
            file_manager,
            lock_table,
            log_manager,
            buffer_manager,
        })
    }

    pub fn new_transaction(&self) -> Result<Transaction, anyhow::Error> {
        Transaction::new(
            self.file_manager.clone(),
            self.log_manager.clone(),
            self.buffer_manager.clone(),
            self.lock_table.clone(),
        )
    }
}
