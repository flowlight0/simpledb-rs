use std::{
    io::Result,
    mem,
    sync::{Arc, Mutex},
};

use crate::{
    file::{BlockId, FileManager},
    page::Page,
};

pub enum LogRecord {
    Start,
}
impl LogRecord {
    fn to_bytes(&self) -> &[u8] {
        match self {
            LogRecord::Start => &['S' as u8],
        }
    }
}

pub struct LogManager {
    file_manager: Arc<Mutex<FileManager>>,
    log_file: String,
    log_page: Page,
    current_block: BlockId,
    append_lock: Mutex<()>,
    latest_log_sequence_number: usize,
    last_saved_log_sequence_number: usize,
}

/// The log manager is responsible for writing log records to the log file.
/// The log file is a sequence of blocks, and the log manager appends log records to the last block.
/// The log records are written to the log file from right to left.
/// The first byte of the block contains the offset to the most recent log record.
impl LogManager {
    pub fn new(file_manager: Arc<Mutex<FileManager>>, log_file: String) -> Result<Self> {
        let mut file_manager_guard = file_manager.lock().unwrap();
        let num_blocks = file_manager_guard.get_num_blocks(&log_file);
        let mut log_page = Page::new(file_manager_guard.block_size);
        let current_block = if num_blocks == 0 {
            // Create the first block of the file
            let block_id = file_manager_guard.append_block(&log_file)?;

            // Currently, page cannot store usize values
            log_page.set_i32(0, file_manager_guard.block_size as i32);
            file_manager_guard.write(&block_id, &log_page)?;
            block_id
        } else {
            // Read the last block of the file
            let current_block = file_manager_guard.get_last_block(&log_file);
            file_manager_guard.read(&current_block, &mut log_page)?;
            current_block
        };

        Ok(Self {
            file_manager: file_manager.clone(),
            log_file,
            log_page,
            current_block,
            append_lock: Mutex::new(()),
            latest_log_sequence_number: 0,
            last_saved_log_sequence_number: 0,
        })
    }

    pub fn append_record(&mut self, log_record: &LogRecord) -> Result<usize> {
        let _lock = self.append_lock.lock().unwrap();
        let mut boundary = self.log_page.get_i32(0) as usize;
        let record_bytes = log_record.to_bytes();
        let record_size = record_bytes.len();
        let boundary_size = mem::size_of::<i32>();
        let mut file_manager = self.file_manager.lock().unwrap();

        if boundary < record_size + boundary_size {
            // The record does not fit in the current block

            // Save the current page into the file
            file_manager.write(&self.current_block, &self.log_page)?;
            self.last_saved_log_sequence_number = self.latest_log_sequence_number;

            // Create a new block
            let new_block = file_manager.append_block(&self.log_file)?;
            self.log_page.set_i32(0, file_manager.block_size as i32);
            file_manager.write(&new_block, &mut self.log_page)?;
            self.current_block = new_block;
            boundary = self.log_page.get_i32(0) as usize;
        }

        let record_position = boundary - record_size;
        self.log_page.set_bytes(record_position, record_bytes);
        self.log_page.set_i32(0, record_position as i32);
        self.latest_log_sequence_number += 1;
        Ok(self.latest_log_sequence_number)
    }

    pub fn flush(&mut self, least_log_sequence_number: usize) -> Result<()> {
        if least_log_sequence_number >= self.last_saved_log_sequence_number {
            self.do_flush()?;
        }
        Ok(())
    }

    fn do_flush(&mut self) -> Result<()> {
        self.file_manager
            .lock()
            .unwrap()
            .write(&self.current_block, &self.log_page)?;
        self.last_saved_log_sequence_number = self.latest_log_sequence_number;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_manager() -> Result<()> {
        let temp_dir = tempfile::tempdir().unwrap().into_path().join("directory");
        let file_manager = FileManager::new(temp_dir, 1024);
        let mut log_manager = LogManager::new(Arc::new(Mutex::new(file_manager)), "log".into())?;
        log_manager.append_record(&LogRecord::Start)?;
        log_manager.flush(1)?;
        Ok(())
    }
}
