use std::{io::Result, mem, sync::Mutex};

use crate::{
    file_manager::{BlockId, FileManager},
    page::Page,
};

enum LogRecord {
    Start,
}
impl LogRecord {
    fn to_bytes(&self) -> &[u8] {
        match self {
            LogRecord::Start => &['S' as u8],
        }
    }
}

struct LogManager {
    file_manager: FileManager,
    log_file: String,
    log_page: Page,
    current_block_slot: usize,
    append_lock: Mutex<()>,
    latest_log_sequence_number: usize,
    last_saved_log_sequence_number: usize,
}

/// The log manager is responsible for writing log records to the log file.
/// The log file is a sequence of blocks, and the log manager appends log records to the last block.
/// The log records are written to the log file from right to left.
/// The first byte of the block contains the offset to the most recent log record.
impl LogManager {
    pub fn new(mut file_manager: FileManager, log_file: String) -> Result<Self> {
        let num_blocks = file_manager.get_num_blocks(&log_file);
        let mut log_page = Page::new(file_manager.block_size);

        let log_file_for_block_id = log_file.clone();
        let current_block_slot = if num_blocks == 0 {
            // Create the first block of the file
            let current_block_slot = file_manager.append_block(&log_file_for_block_id)?;

            // Currently, page cannot store usize values
            log_page.set_i32(0, file_manager.block_size as i32);
            file_manager.write(
                &BlockId::new(&log_file_for_block_id, current_block_slot),
                &log_page,
            )?;
            current_block_slot
        } else {
            // Read the last block of the file
            let current_block = BlockId::new(&log_file_for_block_id, num_blocks - 1);
            file_manager.read(&current_block, &mut log_page)?;
            current_block.block_slot
        };

        Ok(Self {
            file_manager,
            log_file,
            log_page,
            current_block_slot,
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

        if boundary < record_size + boundary_size {
            // The record does not fit in the current block

            // Save the current page into the file
            self.file_manager.write(
                &BlockId::new(&self.log_file, self.current_block_slot),
                &self.log_page,
            )?;
            self.last_saved_log_sequence_number = self.latest_log_sequence_number;

            // Create a new block
            let new_block_slot = self.file_manager.append_block(&self.log_file)?;
            self.log_page
                .set_i32(0, self.file_manager.block_size as i32);
            self.file_manager.write(
                &BlockId::new(&self.log_file, new_block_slot),
                &mut self.log_page,
            )?;
            self.current_block_slot = new_block_slot;
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
        self.file_manager.write(
            &BlockId::new(&self.log_file, self.current_block_slot),
            &self.log_page,
        )?;
        self.last_saved_log_sequence_number = self.latest_log_sequence_number;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_manager() -> Result<()> {
        let file_manager = FileManager::new("data".into(), 1024);
        let mut log_manager = LogManager::new(file_manager, "log".into())?;
        log_manager.append_record(&LogRecord::Start)?;
        log_manager.flush(1)?;
        Ok(())
    }
}
