use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use log::info;

use crate::{
    buffer::BufferManager,
    errors::TransactionError,
    file::FileManager,
    log::manager::LogManager,
    metadata::MetadataManager,
    planner::{
        basic_query_planner::BasicQueryPlanner, index_update_planner::IndexUpdatePlanner, Planner,
    },
    tx::{concurrency::LockTable, transaction::Transaction},
};

pub struct SimpleDB {
    pub file_manager: Arc<Mutex<FileManager>>,
    lock_table: Arc<Mutex<LockTable>>,
    log_manager: Arc<Mutex<LogManager>>,
    buffer_manager: Arc<Mutex<BufferManager>>,
    pub metadata_manager: Arc<Mutex<MetadataManager>>,
    pub planner: Arc<Mutex<Planner>>,
}

impl SimpleDB {
    pub fn new(
        directory: PathBuf,
        block_size: usize,
        num_buffers: usize,
    ) -> Result<SimpleDB, TransactionError> {
        let file_manager = Arc::new(Mutex::new(FileManager::new(directory, block_size)));
        let lock_table = Arc::new(Mutex::new(LockTable::new(10)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(
            file_manager.clone(),
            log_manager.clone(),
            num_buffers,
        )));

        let tx = Arc::new(Mutex::new(Transaction::new(
            file_manager.clone(),
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?));

        let is_new = file_manager.lock().unwrap().is_new;
        if is_new {
            info!("Creating new database");
        } else {
            info!("Recovering new database");
            tx.lock().unwrap().recover()?;
        }
        let metadata_manager = Arc::new(Mutex::new(MetadataManager::new(is_new, tx.clone())?));

        tx.lock().unwrap().commit()?;

        let query_planner = Box::new(BasicQueryPlanner::new(metadata_manager.clone()));
        let update_planner = Box::new(IndexUpdatePlanner::new(metadata_manager.clone()));
        let planner = Arc::new(Mutex::new(Planner::new(query_planner, update_planner)));

        Ok(SimpleDB {
            file_manager,
            lock_table,
            log_manager,
            buffer_manager,
            metadata_manager,
            planner,
        })
    }

    pub fn new_transaction(&self) -> Result<Transaction, TransactionError> {
        Transaction::new(
            self.file_manager.clone(),
            self.log_manager.clone(),
            self.buffer_manager.clone(),
            self.lock_table.clone(),
        )
    }
}
