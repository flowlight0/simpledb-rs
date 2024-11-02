struct TableScan {}

impl TableScan {
    pub fn new() -> TableScan {
        TableScan {}
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::Ok;

    use crate::{buffer::BufferManager, file::FileManager, log::manager::LogManager};

    use super::*;

    #[test]
    fn test_table_scan() -> Result<(), anyhow::Error> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let block_size = 256;
        let file_manager: Arc<Mutex<FileManager>> =
            Arc::new(Mutex::new(FileManager::new(temp_dir, block_size)));
        let lock_table = Arc::new(Mutex::new(LockTable::new(10)));
        let log_manager = LogManager::new(file_manager.clone(), "log".into())?;
        let log_manager = Arc::new(Mutex::new(log_manager));
        let buffer_manager = Arc::new(Mutex::new(BufferManager::new(
            file_manager.clone(),
            log_manager.clone(),
            3,
        )));
        let block = file_manager.lock().unwrap().append_block("testfile")?;

        let mut tx = Transaction::new(
            log_manager.clone(),
            buffer_manager.clone(),
            lock_table.clone(),
        )?;

        let table_scan = TableScan::new(tx, "testtable", layout);
        table_scan.before_first()?;
        for i in 0..50 {
            table_scan.insert()?;
        }

        Ok(())
    }
}
