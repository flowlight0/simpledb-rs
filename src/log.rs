use std::io::Result;

use crate::{block_id::BlockId, file_manager::FileManager, page::Page};

enum LogRecord {
    Start,
}

struct LogManager {
    file_manager: FileManager,
    log_file: String,
    log_page: Page,
    current_block_slot: usize,
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
        let current_block = if num_blocks == 0 {
            // Create the first block of the file
            let current_block = file_manager.append_block(&log_file_for_block_id);

            // Currently, page cannot store usize values
            log_page.set_i32(0, file_manager.block_size as i32);
            file_manager.write(&current_block, &log_page)?;
            current_block
        } else {
            // Read the last block of the file
            let current_block = BlockId::new(&log_file_for_block_id, num_blocks - 1);
            file_manager.read(&current_block, &mut log_page)?;
            current_block
        };

        Ok(Self {
            file_manager,
            log_file,
            log_page,
            current_block_slot: current_block.block_slot,
        })
    }

    pub fn append_record(&mut self, log_record: &LogRecord) -> Result<()> {
        unimplemented!()
    }

    pub fn flush(&mut self) -> Result<()> {
        unimplemented!()
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
        log_manager.flush()?;
        Ok(())
    }
}
