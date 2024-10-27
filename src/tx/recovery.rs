use crate::{
    buffer::BufferManager,
    log::{self, LogManager, LogRecord},
    tx::transaction::Transaction,
};

use std::{
    collections::HashSet,
    io::Result,
    sync::{Arc, Mutex},
};

pub struct RecoveryManager {
    buffer_manager: Arc<Mutex<BufferManager>>,
    log_manager: Arc<Mutex<LogManager>>,
    transaction: Arc<Mutex<Transaction>>,
    transaction_id: usize,
}

impl RecoveryManager {
    pub fn new(
        transaction: Arc<Mutex<Transaction>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
        log_manager: Arc<Mutex<LogManager>>,
        transaction_id: usize,
    ) -> Result<Self> {
        let _log_sequence_number = {
            let mut log_manager = log_manager.lock().unwrap();
            log_manager.append_record(&LogRecord::Start(transaction_id))?
        };

        Ok(RecoveryManager {
            buffer_manager,
            log_manager,
            transaction_id,
            transaction,
        })
    }

    pub fn commit(&mut self) -> Result<()> {
        let buffer_manager = self.buffer_manager.lock().unwrap();
        let mut log_manager = self.log_manager.lock().unwrap();
        buffer_manager.flush_all(self.transaction_id)?;
        let log_sequence_number =
            log_manager.append_record(&LogRecord::Commit(self.transaction_id))?;
        log_manager.flush(log_sequence_number)?;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.do_rollback()?;
        let buffer_manager = self.buffer_manager.lock().unwrap();
        buffer_manager.flush_all(self.transaction_id)?;
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_sequence_number =
            log_manager.append_record(&LogRecord::Rollback(self.transaction_id))?;
        log_manager.flush(log_sequence_number)?;
        Ok(())
    }

    // This is called when the transaction is instantiated.
    // "Quiescent Checkpointing" is currently implemented
    // This method is not thread-safe.
    pub fn recover(&mut self) -> Result<()> {
        self.do_recover()?;
        let buffer_manager = self.buffer_manager.lock().unwrap();
        buffer_manager.flush_all(self.transaction_id)?;
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_sequence_number =
            log_manager.append_record(&LogRecord::Checkpoint(self.transaction_id))?;
        log_manager.flush(log_sequence_number)?;
        Ok(())
    }

    fn do_rollback(&mut self) -> Result<()> {
        let log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter();
        let mut transaction = self.transaction.lock().unwrap();

        for log_record in log_iter {
            if log_record.get_transaction_id() == self.transaction_id {
                transaction.undo(log_record)?;
                if let LogRecord::Start(_) = log_record {
                    break;
                }
            }
        }
        Ok(())
    }

    fn do_recover(&mut self) -> Result<()> {
        let log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter();
        let mut finshed_transactions = HashSet::new();
        let mut transaction = self.transaction.lock().unwrap();

        for log_record in log_iter {
            match log_record {
                LogRecord::Start(transaction_id) | LogRecord::Commit(transaction_id) => {
                    finshed_transactions.insert(transaction_id);
                }
                LogRecord::Checkpoint(_) => {
                    break;
                }
                _ => {
                    if log_record.get_transaction_id() == self.transaction_id {
                        transaction.undo(log_record)?;
                    }
                }
            }
        }
        Ok(())
    }
}
