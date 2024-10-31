use crate::{
    buffer::{Buffer, BufferManager},
    log::{LogManager, LogRecord},
    tx::transaction::Transaction,
};

use std::{
    cell::RefCell,
    collections::HashSet,
    io::Result,
    rc::Weak,
    sync::{Arc, Mutex},
};

pub struct RecoveryManager {
    buffer_manager: Arc<Mutex<BufferManager>>,
    log_manager: Arc<Mutex<LogManager>>,
    transaction_id: usize,
    transaction: RefCell<Weak<Transaction>>,
}

impl RecoveryManager {
    pub fn new(
        // transaction: Arc<Mutex<Transaction>>,
        buffer_manager: Arc<Mutex<BufferManager>>,
        log_manager: Arc<Mutex<LogManager>>,
        transaction_id: usize,
        transaction: RefCell<Weak<Transaction>>,
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

    pub fn commit(&mut self, transaction_id: usize) -> Result<()> {
        let buffer_manager = self.buffer_manager.lock().unwrap();
        buffer_manager.flush_all(transaction_id)?;

        let mut log_manager = self.log_manager.lock().unwrap();
        let log_sequence_number = log_manager.append_record(&LogRecord::Commit(transaction_id))?;
        log_manager.flush(log_sequence_number)?;
        Ok(())
    }

    pub fn rollback(&mut self, transaction_id: usize) -> Result<()> {
        self.do_rollback()?;
        let buffer_manager = self.buffer_manager.lock().unwrap();
        buffer_manager.flush_all(transaction_id)?;
        let mut log_manager = self.log_manager.lock().unwrap();
        let log_sequence_number =
            log_manager.append_record(&LogRecord::Rollback(transaction_id))?;
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

    pub fn set_i32(&self, buffer: &mut Buffer, offset: usize, value: i32) -> Result<usize> {
        let block = buffer.block.expect("buffer must be assigned to a block");
        let old_value = buffer.page.get_i32(offset);
        let record = LogRecord::SetI32(self.transaction_id, block, offset, old_value, value);

        let mut log_manager = self.log_manager.lock().unwrap();
        log_manager.append_record(&record)
    }

    fn do_rollback(&mut self) -> Result<()> {
        let log_manager = self.log_manager.lock().unwrap();
        let log_iter = log_manager.get_backward_iter();

        for log_record in log_iter {
            if log_record.get_transaction_id() == self.transaction_id {
                // transaction.undo(&log_record)?;
                self.transaction
                    .borrow()
                    .upgrade()
                    .unwrap()
                    .undo(&log_record)?;
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
                        self.transaction
                            .borrow()
                            .upgrade()
                            .unwrap()
                            .undo(&log_record)?;
                    }
                }
            }
        }
        Ok(())
    }
}
