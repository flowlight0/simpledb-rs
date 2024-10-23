// use crate::{
//     buffer::BufferManager,
//     log::{LogManager, LogRecord},
// };

// struct RecoveryManager {
//     buffer_manager: BufferManager,
//     log_manager: LogManager,
//     transaction_id: usize,
// }

// impl RecoveryManager {
//     pub fn new() -> Self {
//         RecoveryManager {
//             buffer_manager: BufferManager::new(),
//             log_manager: LogManager::new(),
//             transaction_id: 0,
//         }
//     }

//     pub fn commit(&self) -> Result<()> {
//         self.buffer_manager.flush_all(self.transaction_id)?;
//         LogRecord::Commit(self.transaction_id).append_record(&self.log_manager)?;
//         self.log_manager.flush(self.transaction_id)?;
//         Ok(())
//     }

//     pub fn rollback() -> Result<()> {
//         self.buffer_manager.rollback(self.transaction_id)?;
//         self.log_manager.rollback(self.transaction_id)?;
//         Ok(())
//     }
// }
