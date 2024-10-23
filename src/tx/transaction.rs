// use std::sync::{Arc, Mutex};

// use crate::{buffer::BufferManager, file::FileManager, log::LogManager};

// struct Transaction {
//     file_manager: Arc<Mutex<FileManager>>,
//     buffer_manager: Arc<Mutex<BufferManager>>,
//     log_manager: Arc<Mutex<LogManager>>,
// }

// impl Transaction {
//     pub fn new(
//         file_manager: Arc<Mutex<FileManager>>,
//         buffer_manager: Arc<Mutex<BufferManager>>,
//         log_manager: Arc<Mutex<LogManager>>,
//     ) -> Self {
//         Transaction {
//             file_manager,
//             buffer_manager,
//             log_manager,
//         }
//     }

//     pub fn commit(&self) -> Result<()> {
//         self.recovery_manager.commit()?;
//         self.concurrency_manager.commit()?;

//         Ok(())
//     }

//     pub fn rollback(&self) -> Result<()> {
//         self.buffer_manager.lock().unwrap().rollback()?;
//         self.log_manager.lock().unwrap().rollback()?;
//         Ok(())
//     }
// }
